<script lang="ts">
  import { untrack } from "svelte";
  import {
    buildAnswers,
    canSubmitAnswers,
    emptyDrafts,
    isAnswered,
    questionTab,
    toggleOption,
    toggleOther,
    type QuestionDraft,
  } from "$lib/domain/questions";
  import type { PendingQuestion } from "$lib/protocol/PendingQuestion";
  import { answerQuestion } from "$lib/runtime";
  import { Button, Pill } from "$lib/ui";
  import Icon, { Check, HelpCircle } from "$lib/ui/icons";

  type Props = { pending: PendingQuestion; agentLabel: string | null };
  let { pending, agentLabel }: Props = $props();

  let drafts = $state<QuestionDraft[]>(untrack(() => emptyDrafts(pending.questions)));
  let tab = $state(0);
  let sending = $state(false);
  const questions = $derived(pending.questions);
  const question = $derived(questions[tab]);
  const draft = $derived(drafts[tab]);
  const ready = $derived(canSubmitAnswers(questions, drafts));

  function update(next: QuestionDraft) {
    drafts = drafts.map((d, i) => (i === tab ? next : d));
  }

  function pick(label: string) {
    update(toggleOption(draft, question, label));
    if (!question.multiSelect && tab < questions.length - 1) tab += 1;
  }

  async function respond(answers: ReturnType<typeof buildAnswers> | null) {
    if (sending) return;
    sending = true;
    const ok = await answerQuestion(pending.requestId, answers);
    if (!ok) sending = false;
  }
</script>

{#if question && draft}
  <div class="question-card" role="group" aria-label="Question from the agent" tabindex="-1" data-pending-card>
    <div class="interaction-kicker">
      <Icon icon={HelpCircle} size={13} />
      <span>{questions.length > 1 ? `${questions.length} questions` : "Question"}</span>
      {#if agentLabel}<Pill tone="info">{agentLabel}</Pill>{/if}
    </div>

    {#if questions.length > 1}
      <div class="question-tabs" role="tablist" aria-label="Questions">
        {#each questions as q, i (i)}
          <button
            type="button"
            role="tab"
            class={`question-tab${i === tab ? " active" : ""}`}
            aria-selected={i === tab}
            onclick={() => (tab = i)}
          >
            {#if isAnswered(drafts[i])}<Icon icon={Check} size={10} strokeWidth={2.4} />{/if}
            <span>{questionTab(q, i)}</span>
          </button>
        {/each}
      </div>
    {/if}

    <p class="question-text">{question.question}</p>
    <div class="question-options" role={question.multiSelect ? "group" : "radiogroup"}>
      {#each question.options as option (option.label)}
        {@const selected = draft.selected.includes(option.label)}
        <button
          type="button"
          class={`question-option${selected ? " selected" : ""}${question.multiSelect ? " multi" : ""}`}
          role={question.multiSelect ? "checkbox" : "radio"}
          aria-checked={selected}
          onclick={() => pick(option.label)}
        >
          <span class="question-option-mark" aria-hidden="true"></span>
          <span class="question-option-text">
            <span class="question-option-label">{option.label}</span>
            {#if option.description}<span class="question-option-desc">{option.description}</span>{/if}
          </span>
        </button>
      {/each}
      <button
        type="button"
        class={`question-option${draft.useOther ? " selected" : ""}${question.multiSelect ? " multi" : ""}`}
        role={question.multiSelect ? "checkbox" : "radio"}
        aria-checked={draft.useOther}
        onclick={() => update(toggleOther(draft, question))}
      >
        <span class="question-option-mark" aria-hidden="true"></span>
        <span class="question-option-text"><span class="question-option-label">Other…</span></span>
      </button>
      {#if draft.useOther}
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="question-other-input"
          placeholder="Type your answer"
          value={draft.other}
          autofocus
          oninput={(e) => update({ ...draft, other: e.currentTarget.value })}
          onkeydown={(e) => {
            if (e.key === "Enter" && ready) void respond(buildAnswers(questions, drafts));
          }}
        />
      {/if}
    </div>

    <div class="interaction-actions">
      <Button variant="accent" disabled={!ready || sending} onclick={() => void respond(buildAnswers(questions, drafts))}>
        Submit
      </Button>
      <Button disabled={sending} onclick={() => void respond(null)}>Dismiss</Button>
      <span class="hint">{question.multiSelect ? "Pick any that apply" : "Pick one"}</span>
    </div>
  </div>
{/if}
