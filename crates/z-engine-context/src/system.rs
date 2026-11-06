//! The layered system prompt, ordered for prompt-cache stability.

use z_engine_prompts::reminders::SKILLS_LISTING;

use crate::environment::{Environment, render_environment};
use crate::instructions::{InstructionDoc, render_instructions};
use crate::sections::PromptSection;
use crate::template::render_template;
use crate::text::single_line;

#[derive(Debug, Clone, Copy)]
pub struct SystemInputs<'a> {
    /// The main system prompt, or a subagent's definition prompt plus preamble.
    pub base_prompt: &'a str,
    pub environment: &'a Environment,
    pub instructions: &'a [InstructionDoc],
    /// `(name, description)` of every skill the agent may load.
    pub skills: &'a [(String, String)],
    pub output_style: Option<&'a str>,
    /// Per-request text; the only section outside the cached prefix.
    pub extra: Option<&'a str>,
}

/// Sections in cache-stable order: base prompt, environment, instructions,
/// skills listing and output style (all cacheable), then `extra`
/// (uncached). Blank sections are omitted because providers reject empty
/// text blocks.
pub fn build_system(inputs: &SystemInputs<'_>) -> Vec<PromptSection> {
    let mut sections = Vec::with_capacity(6);
    let mut push = |text: &str, cacheable: bool| {
        let text = text.trim();
        if !text.is_empty() {
            sections.push(PromptSection {
                text: text.to_string(),
                cacheable,
            });
        }
    };
    push(inputs.base_prompt, true);
    push(&render_environment(inputs.environment), true);
    push(
        &render_instructions(inputs.instructions).unwrap_or_default(),
        true,
    );
    push(&render_skills(inputs.skills), true);
    push(inputs.output_style.unwrap_or_default(), true);
    push(inputs.extra.unwrap_or_default(), false);
    sections
}

/// [`SKILLS_LISTING`] with one `- name: description` line per skill; empty
/// when there are no skills.
fn render_skills(skills: &[(String, String)]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = skills
        .iter()
        .map(|(name, description)| {
            let description = single_line(description);
            if description.is_empty() {
                format!("- {}", name.trim())
            } else {
                format!("- {}: {description}", name.trim())
            }
        })
        .collect();
    render_template(SKILLS_LISTING, &[("skills", &lines.join("\n"))])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Environment {
        Environment {
            cwd: "/p".into(),
            project_root: "/p".into(),
            platform: "linux".into(),
            shell: "bash".into(),
            date: "2026-09-23".into(),
            model: "m".into(),
            ..Environment::default()
        }
    }

    #[test]
    fn sections_follow_cache_order_and_only_extra_is_uncached() {
        let env = env();
        let docs = [InstructionDoc {
            label: "Project instructions (AGENTS.md)".into(),
            path: "/p/AGENTS.md".into(),
            content: "Use tabs.".into(),
        }];
        let skills = [
            ("pdf".to_string(), "Fill PDF\nforms.".to_string()),
            ("bare".to_string(), String::new()),
        ];
        let sections = build_system(&SystemInputs {
            base_prompt: "You are HyperForm.\n",
            environment: &env,
            instructions: &docs,
            skills: &skills,
            output_style: Some("Be brief."),
            extra: Some("Turn note."),
        });
        assert_eq!(sections.len(), 6);
        assert_eq!(sections[0].text, "You are HyperForm.");
        assert!(sections[1].text.contains("Working directory: /p"));
        assert!(sections[2].text.contains("Use tabs."));
        assert!(sections[3].text.contains("- pdf: Fill PDF forms.\n- bare"));
        assert_eq!(sections[4].text, "Be brief.");
        assert_eq!(sections[5], PromptSection::uncached("Turn note."));
        assert!(sections[..5].iter().all(|section| section.cacheable));
        assert!(sections.iter().all(|section| !section.text.contains("{{")));
    }

    #[test]
    fn blank_and_absent_sections_are_skipped() {
        let env = env();
        let sections = build_system(&SystemInputs {
            base_prompt: "Base",
            environment: &env,
            instructions: &[],
            skills: &[],
            output_style: Some("  "),
            extra: None,
        });
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0], PromptSection::cached("Base"));
        assert!(sections[1].text.contains("<env>"));
    }
}
