import type { Theme } from "vitepress";
import DefaultTheme from "vitepress/theme-without-fonts";
import { h } from "vue";
import HomeOrb from "./HomeOrb.vue";
import "./style.css";

export default {
  extends: DefaultTheme,
  Layout: () => h(DefaultTheme.Layout, null, { "home-hero-image": () => h(HomeOrb) }),
} satisfies Theme;
