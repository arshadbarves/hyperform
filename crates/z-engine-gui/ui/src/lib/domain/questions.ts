import type { Question } from "../protocol/Question";
import type { QuestionAnswer } from "../protocol/QuestionAnswer";

/** Local state for one `AskUserQuestion` tab. */
export interface QuestionDraft {
  selected: string[];
  useOther: boolean;
  other: string;
}

export function emptyDrafts(questions: Question[]): QuestionDraft[] {
  return questions.map(() => ({ selected: [], useOther: false, other: "" }));
}

/** Single-select replaces the pick (and leaves "Other"); multi-select toggles. */
export function toggleOption(draft: QuestionDraft, question: Question, label: string): QuestionDraft {
  if (!question.multiSelect) return { ...draft, selected: [label], useOther: false };
  const selected = draft.selected.includes(label)
    ? draft.selected.filter((l) => l !== label)
    : [...draft.selected, label];
  return { ...draft, selected };
}

export function toggleOther(draft: QuestionDraft, question: Question): QuestionDraft {
  if (!question.multiSelect) return { ...draft, useOther: true, selected: [] };
  return { ...draft, useOther: !draft.useOther };
}

export function draftAnswers(draft: QuestionDraft): string[] {
  const other = draft.useOther ? draft.other.trim() : "";
  return other ? [...draft.selected, other] : [...draft.selected];
}

export function isAnswered(draft: QuestionDraft | undefined): boolean {
  return draft !== undefined && draftAnswers(draft).length > 0;
}

export function canSubmitAnswers(questions: Question[], drafts: QuestionDraft[]): boolean {
  return questions.length > 0 && questions.every((_, i) => isAnswered(drafts[i]));
}

export function buildAnswers(questions: Question[], drafts: QuestionDraft[]): QuestionAnswer[] {
  return questions.map((q, i) => ({ question: q.question, answers: draftAnswers(drafts[i]) }));
}

/** Tab label: the short header, else a clipped question. */
export function questionTab(question: Question, index: number): string {
  const header = question.header.trim();
  if (header) return header;
  const text = question.question.trim();
  if (!text) return `Question ${index + 1}`;
  return text.length > 16 ? `${text.slice(0, 15)}…` : text;
}
