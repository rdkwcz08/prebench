// app/ui/main.ts — Point d'entrée de l'interface.
import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';
import { theme } from './lib/theme.svelte';

const target = document.getElementById('app');
if (target === null) {
  throw new Error('élément #app introuvable');
}
theme.init();
export default mount(App, { target });
