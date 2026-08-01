use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use thiserror::Error;

use crate::domain::{
    CycleState, DecisionContext, DiscussionEvent, MemberFailure, ReplyContext, SpeakerDecision,
    StopReason,
};
use crate::providers::ModelProvider;
use crate::repository::{RepositoryError, WorkspaceRepository};

const MESSAGE_LIMIT: usize = 12;

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

pub struct DiscussionScheduler {
    repository: WorkspaceRepository,
    providers: HashMap<String, Arc<dyn ModelProvider>>,
}

impl DiscussionScheduler {
    pub fn new(
        repository: WorkspaceRepository,
        providers: HashMap<String, Arc<dyn ModelProvider>>,
    ) -> Self {
        Self {
            repository,
            providers,
        }
    }

    pub async fn handle_event(
        &self,
        initial_event: DiscussionEvent,
    ) -> Result<CycleState, SchedulerError> {
        let mut queue = VecDeque::from([initial_event]);
        let mut model_message_count = 0;
        let mut failures = Vec::new();
        let mut failed_members = HashSet::new();

        while let Some(event) = queue.pop_front() {
            if model_message_count >= MESSAGE_LIMIT {
                break;
            }
            let thread = self.repository.load_thread(&event.conversation_id).await?;
            let trigger_author = thread
                .messages
                .iter()
                .find(|message| message.id == event.trigger_message_id)
                .and_then(|message| message.author_id.as_deref());
            let last_model_author = thread
                .messages
                .iter()
                .rev()
                .find(|message| message.author_kind == "model")
                .and_then(|message| message.author_id.as_deref());

            let mut candidates = Vec::new();
            for member in &thread.members {
                let explicitly_mentioned = event.mentioned_member_id.as_deref() == Some(&member.id);
                if event.mentioned_member_id.is_some() && !explicitly_mentioned {
                    continue;
                }
                if !explicitly_mentioned
                    && (trigger_author == Some(&member.id) || last_model_author == Some(&member.id))
                {
                    continue;
                }

                let Some(provider) = self.providers.get(&member.id) else {
                    add_failure(
                        &mut failures,
                        &mut failed_members,
                        &member.id,
                        "模型适配器未配置",
                    );
                    continue;
                };
                let context = DecisionContext {
                    conversation_id: event.conversation_id.clone(),
                    trigger_message_id: event.trigger_message_id.clone(),
                    member: member.clone(),
                    visible_messages: thread.messages.clone(),
                };
                match provider.decide(context).await {
                    Ok(SpeakerDecision::Reply { reason, priority }) => {
                        self.repository
                            .record_event(
                                &event.conversation_id,
                                &event.trigger_message_id,
                                &member.id,
                                "decision",
                                "reply",
                                Some(&reason),
                            )
                            .await?;
                        candidates.push((priority, member.id.clone()));
                    }
                    Ok(SpeakerDecision::Silent { reason }) => {
                        self.repository
                            .record_event(
                                &event.conversation_id,
                                &event.trigger_message_id,
                                &member.id,
                                "decision",
                                "silent",
                                Some(&reason),
                            )
                            .await?;
                    }
                    Err(error) => {
                        self.repository
                            .record_event(
                                &event.conversation_id,
                                &event.trigger_message_id,
                                &member.id,
                                "decision",
                                "failed",
                                Some(&error.to_string()),
                            )
                            .await?;
                        add_failure(
                            &mut failures,
                            &mut failed_members,
                            &member.id,
                            &error.to_string(),
                        );
                    }
                }
            }

            candidates
                .sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
            for (_, member_id) in candidates {
                if model_message_count >= MESSAGE_LIMIT {
                    break;
                }
                let latest = self.repository.load_thread(&event.conversation_id).await?;
                let Some(member) = latest.members.iter().find(|member| member.id == member_id)
                else {
                    continue;
                };
                let Some(provider) = self.providers.get(&member_id) else {
                    continue;
                };
                let reply_context = ReplyContext {
                    conversation_id: event.conversation_id.clone(),
                    member: member.clone(),
                    visible_messages: latest.messages,
                };
                match provider.reply(reply_context).await {
                    Ok(reply) => {
                        let message = self
                            .repository
                            .append_message(
                                &event.conversation_id,
                                "model",
                                Some(&member_id),
                                &reply.content,
                            )
                            .await?;
                        self.repository
                            .record_event(
                                &event.conversation_id,
                                &event.trigger_message_id,
                                &member_id,
                                "reply",
                                "completed",
                                None,
                            )
                            .await?;
                        model_message_count += 1;
                        queue.push_back(DiscussionEvent::new(
                            event.conversation_id.clone(),
                            message.id,
                        ));
                    }
                    Err(error) => {
                        add_failure(
                            &mut failures,
                            &mut failed_members,
                            &member_id,
                            &error.to_string(),
                        );
                    }
                }
            }
        }

        Ok(CycleState {
            model_message_count,
            stop_reason: if model_message_count >= MESSAGE_LIMIT {
                StopReason::MessageLimit
            } else {
                StopReason::AllSilent
            },
            failures,
        })
    }
}

fn add_failure(
    failures: &mut Vec<MemberFailure>,
    failed_members: &mut HashSet<String>,
    member_id: &str,
    message: &str,
) {
    if failed_members.insert(member_id.to_owned()) {
        failures.push(MemberFailure {
            member_id: member_id.to_owned(),
            message: message.to_owned(),
        });
    }
}
