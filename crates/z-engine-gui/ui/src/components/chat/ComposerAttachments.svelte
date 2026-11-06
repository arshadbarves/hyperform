<script lang="ts">
  import { attachmentLabel, fileExtension, imageSrc } from "$lib/domain/attachments";
  import type { Attachment } from "$lib/protocol/Attachment";
  import Icon, { FileText, X } from "$lib/ui/icons";

  type Props = { attachments: Attachment[]; onRemove: (index: number) => void };
  let { attachments, onRemove }: Props = $props();
</script>

{#if attachments.length > 0}
  <div class="attachments">
    {#each attachments as attachment, i (i)}
      {#if attachment.type === "image"}
        <span class="attachment img-chip">
          <button class="att-x" title="Remove image" onclick={() => onRemove(i)} type="button">
            <Icon icon={X} size={9} strokeWidth={2.4} />
          </button>
          <img src={imageSrc(attachment)} alt={`image ${i + 1}`} />
        </span>
      {:else}
        <span class="attachment" title={attachment.path}>
          <button class="att-x" title={`Remove ${attachment.path}`} onclick={() => onRemove(i)} type="button">
            <Icon icon={X} size={9} strokeWidth={2.4} />
          </button>
          <span class="att-icon"><Icon icon={FileText} size={14} strokeWidth={1.8} /></span>
          <span class="att-text">
            <span class="att-name">{attachmentLabel(attachment)}</span>
            <span class="att-ext">{fileExtension(attachment.path)}</span>
          </span>
        </span>
      {/if}
    {/each}
  </div>
{/if}
