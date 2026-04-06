/// <reference types="svelte" />
/// <reference types="vite/client" />

declare const __APP_VERSION__: string;

declare module '*.svelte' {
  import type { ComponentType } from 'svelte';
  const component: ComponentType;
  export default component;
}