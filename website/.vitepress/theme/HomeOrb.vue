<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, useId } from "vue";

/** The companion orb at rest. Decorative; it holds still under Reduce Motion. */
const id = useId();
const blinking = ref(false);
let timer: ReturnType<typeof setTimeout> | undefined;

function scheduleBlink(): void {
  timer = setTimeout(
    () => {
      blinking.value = !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      timer = setTimeout(() => {
        blinking.value = false;
        scheduleBlink();
      }, 130);
    },
    2600 + Math.random() * 3400,
  );
}

onMounted(scheduleBlink);
onBeforeUnmount(() => clearTimeout(timer));
</script>

<template>
  <svg class="home-orb" :class="{ 'is-blinking': blinking }" viewBox="0 0 30 30" aria-hidden="true">
    <defs>
      <radialGradient :id="`${id}-body`" cx="38%" cy="30%" r="80%">
        <stop offset="0%" class="stop-light" />
        <stop offset="55%" class="stop-pearl" />
        <stop offset="100%" class="stop-edge" />
      </radialGradient>
      <filter :id="`${id}-glow`" x="-60%" y="-60%" width="220%" height="220%">
        <feGaussianBlur stdDeviation="2.4" />
      </filter>
    </defs>
    <g class="body">
      <circle class="aura" cx="15" cy="15" r="10" :filter="`url(#${id}-glow)`" />
      <circle cx="15" cy="15" r="10" :fill="`url(#${id}-body)`" />
      <circle class="rim" cx="15" cy="15" r="9.7" />
      <ellipse class="shine" cx="11.6" cy="10.2" rx="3.2" ry="1.8" transform="rotate(-28 11.6 10.2)" />
      <ellipse class="eye" cx="12" cy="15.6" rx="1.3" ry="1.95" />
      <ellipse class="eye" cx="18" cy="15.6" rx="1.3" ry="1.95" />
    </g>
  </svg>
</template>

<style scoped>
.home-orb {
  position: absolute;
  top: 50%;
  left: 50%;
  width: 192px;
  height: 192px;
  overflow: visible;
  transform: translate(-50%, -50%);
}

@media (min-width: 640px) {
  .home-orb {
    width: 256px;
    height: 256px;
  }
}

@media (min-width: 960px) {
  .home-orb {
    width: 288px;
    height: 288px;
  }
}

.stop-light {
  stop-color: var(--zen-orb-light);
}
.stop-pearl {
  stop-color: var(--zen-orb-pearl);
}
.stop-edge {
  stop-color: var(--zen-orb-edge);
}
.aura {
  fill: var(--zen-orb-aura);
  opacity: var(--zen-orb-aura-opacity);
}
.rim {
  fill: none;
  stroke: var(--zen-orb-rim);
  stroke-width: 0.6;
}
.shine {
  fill: var(--zen-orb-light);
  opacity: 0.75;
}
.eye {
  fill: var(--zen-orb-eye);
  transform-box: fill-box;
  transform-origin: center;
  transition: transform 160ms cubic-bezier(0.16, 1, 0.3, 1);
}
.is-blinking .eye {
  transform: scaleY(0.1);
  transition-duration: 60ms;
}

.body {
  transform-box: fill-box;
  transform-origin: 50% 100%;
  animation: breathe 4.2s cubic-bezier(0.65, 0, 0.35, 1) infinite;
}

@keyframes breathe {
  0%,
  100% {
    scale: 1;
  }
  50% {
    scale: 1.035;
  }
}

@media (prefers-reduced-motion: reduce) {
  .body {
    animation: none;
  }
  .eye {
    transition: none;
  }
}
</style>
