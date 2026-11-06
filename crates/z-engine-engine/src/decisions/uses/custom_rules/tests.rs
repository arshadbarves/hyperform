//! Custom rules only ask, notify or remind: an Allow may become an Ask,
//! Ask and Deny never change, bypass mode only notifies, nothing happens in
//! `off`, `shadow` or with a model that is down or unsure, and an
//! untrusted project's rules never run.

use serde_json::json;
use z_engine_config::{DecisionRule, FeatureId, FeatureMode};
use z_engine_policy::Decision;
use z_engine_protocol::{AgentId, CallId, PermissionMode, ToolResultPart, ToolStatus};

use super::judge::question_names;
use super::tool::matches;
use crate::batch::ToolCall;
use crate::decisions::seams::{annotate_result, at_turn_start, review_call};
use crate::decisions::uses::scripted::{
    Events, Scripted, install, main_run, notices, session_with,
};
use crate::session::SessionHandle;

const FEATURE: FeatureId = FeatureId::DecisionsCustomRules;

fn rules_toml(rules: &[(&str, &str, &str)]) -> String {
    let mut toml = String::from("schema = 2\n");
    for (name, event, action) in rules {
        let matcher = if *event == "PreToolUse" {
            "matcher = \"Bash\"\n"
        } else {
            ""
        };
        toml.push_str(&format!(
            "\n[[decisions.rules]]\nname = \"{name}\"\nevent = \"{event}\"\n{matcher}\
             question = \"Does it touch production?\"\naction = \"{action}\"\n"
        ));
    }
    toml
}

async fn with_rules(
    dir: &std::path::Path,
    rules: &[(&str, &str, &str)],
) -> (SessionHandle, Events) {
    session_with(dir, Some(&rules_toml(rules))).await
}

fn bash() -> ToolCall {
    ToolCall {
        id: CallId::from("call_1"),
        name: "Bash".into(),
        input: json!({ "command": "kubectl apply -f prod.yaml" }),
    }
}

fn allow() -> Decision {
    Decision::Allow {
        reason: "allow rule".into(),
    }
}

/// A model confidently answering "yes" to the rule named `name`.
fn yes(name: &str) -> Scripted {
    Scripted::default().yes(&format!("rule_{name}"), true)
}

#[test]
fn matchers_and_question_names_follow_hooks_and_labels() {
    assert!(matches(None, "Bash") && matches(Some("*"), "Read"));
    assert!(matches(Some("Bash|Write"), "Write"));
    assert!(!matches(Some("Bash"), "BashOutput"));
    assert!(!matches(Some("("), "Bash"));
    let rule = |name: Option<&str>, question: &str| DecisionRule {
        name: name.map(str::to_string),
        question: question.into(),
        ..DecisionRule::default()
    };
    let rules = [
        rule(Some("Prod deploys"), "x"),
        rule(None, "Does it touch production?"),
        rule(Some("prod deploys!"), "y"),
    ];
    assert_eq!(
        question_names(&rules),
        [
            "rule_prod_deploys",
            "rule_does_it_touch_production",
            "rule_prod_deploys_2"
        ]
    );
}

#[tokio::test]
async fn a_fired_ask_rule_turns_allow_into_ask_and_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = with_rules(dir.path(), &[("prod", "PreToolUse", "ask")]).await;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, yes("prod"));
    let decision = review_call(&ctx, &bash(), allow()).await;
    assert!(
        matches!(&decision, Decision::Ask { reason, can_persist: false, .. }
            if reason.contains("Does it touch production? yes")),
        "{decision:?}"
    );
    let deny = Decision::Deny {
        reason: "deny rule".into(),
    };
    assert_eq!(review_call(&ctx, &bash(), deny.clone()).await, deny);
    let read = ToolCall {
        name: "Read".into(),
        input: json!({ "file_path": "/x" }),
        ..bash()
    };
    assert_eq!(review_call(&ctx, &read, allow()).await, allow(), "matcher");
    let no = Scripted::default().yes("rule_prod", false);
    install(&handle, FEATURE, FeatureMode::On, no);
    assert_eq!(review_call(&ctx, &bash(), allow()).await, allow());
    handle.close("test").await;
}

#[tokio::test]
async fn off_down_unsure_and_shadow_change_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let rules = [
        ("prod", "PreToolUse", "ask"),
        ("note", "UserPromptSubmit", "remind"),
    ];
    let (handle, events) = with_rules(dir.path(), &rules).await;
    let ctx = main_run(&handle);
    let both = yes("prod").yes("rule_note", true);
    install(&handle, FEATURE, FeatureMode::On, both.clone());
    at_turn_start(&ctx, "deploy it").await;
    assert_eq!(
        handle.core.reminders.take(&AgentId::main()).len(),
        1,
        "sanity"
    );
    for (mode, model) in [
        (FeatureMode::Off, both.clone()),
        (FeatureMode::On, Scripted::down()),
        (FeatureMode::On, Scripted::default()),
        (FeatureMode::Shadow, both),
    ] {
        install(&handle, FEATURE, mode, model);
        assert_eq!(
            review_call(&ctx, &bash(), allow()).await,
            allow(),
            "{mode:?}"
        );
        at_turn_start(&ctx, "deploy it").await;
        assert!(
            handle.core.reminders.take(&AgentId::main()).is_empty(),
            "{mode:?}"
        );
    }
    assert!(notices(&events).is_empty());
    handle.close("test").await;
}

#[tokio::test]
async fn notice_rules_and_bypass_mode_only_notify() {
    let dir = tempfile::tempdir().unwrap();
    let rules = [
        ("prod", "PreToolUse", "ask"),
        ("watch", "PreToolUse", "notice"),
    ];
    let (handle, events) = with_rules(dir.path(), &rules).await;
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, yes("watch"));
    assert_eq!(review_call(&ctx, &bash(), allow()).await, allow());
    handle
        .core
        .with_state(|state| state.mode = PermissionMode::Bypass);
    install(&handle, FEATURE, FeatureMode::On, yes("prod"));
    assert_eq!(review_call(&ctx, &bash(), allow()).await, allow());
    let notices = notices(&events);
    assert_eq!(notices.len(), 2, "{notices:?}");
    assert!(
        notices
            .iter()
            .all(|text| text.starts_with("Decision rule on Bash"))
    );
    handle.close("test").await;
}

#[tokio::test]
async fn remind_and_prompt_rules_reach_the_agent() {
    let dir = tempfile::tempdir().unwrap();
    let rules = [
        ("after", "PreToolUse", "remind"),
        ("check", "UserPromptSubmit", "ask"),
    ];
    let (handle, _) = with_rules(dir.path(), &rules).await;
    let ctx = main_run(&handle);
    install(
        &handle,
        FEATURE,
        FeatureMode::On,
        yes("after").yes("rule_check", true),
    );
    let output = ToolResultPart::Text {
        text: "applied".into(),
    };
    let mut content = vec![output.clone()];
    annotate_result(&ctx, &bash(), ToolStatus::Ok, &mut content).await;
    assert_eq!(content.len(), 2);
    assert!(
        matches!(&content[0], ToolResultPart::Text { text } if text.contains("Tool call: Bash"))
    );
    assert_eq!(content[1], output);
    at_turn_start(&ctx, "deploy it").await;
    let reminders = handle.core.reminders.take(&AgentId::main());
    assert_eq!(reminders.len(), 1);
    assert!(reminders[0].contains("AskUserQuestion"), "{reminders:?}");
    handle.close("test").await;
}

#[tokio::test]
async fn an_untrusted_projects_rules_never_run() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project/.z-engine");
    std::fs::create_dir_all(&project).unwrap();
    let rules = rules_toml(&[("prod", "PreToolUse", "ask")]);
    std::fs::write(project.join("settings.toml"), rules).unwrap();
    let (handle, _) = session_with(dir.path(), None).await;
    assert!(handle.core.settings().settings.decisions.rules.is_empty());
    let ctx = main_run(&handle);
    install(&handle, FEATURE, FeatureMode::On, yes("prod"));
    assert_eq!(review_call(&ctx, &bash(), allow()).await, allow());
    handle.close("test").await;
}
