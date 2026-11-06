<script lang="ts">
  import type { Starter } from "$lib/domain/starters";
  import Icon, { BadgeCheck, Book, GitCompare, Search, Sparkles, Wrench } from "$lib/ui/icons";

  /** The glyph beside a starter prompt. */
  type Props = { kind: Starter["kind"] };
  let { kind }: Props = $props();

  const ICONS = { review: GitCompare, docs: Book, explain: Search, fix: Wrench, test: BadgeCheck, clean: Sparkles } as const;
</script>

<span class="starter-icon" aria-hidden="true"><Icon icon={ICONS[kind]} size={14} strokeWidth={1.8} /></span>
