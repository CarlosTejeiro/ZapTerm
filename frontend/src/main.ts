import { mount } from 'svelte'
import App from './App.svelte'
// Bundled brand fonts — UI (Geist) + terminal (Cascadia Code, JetBrains Mono). Variable
// weights, self-hosted so the chrome looks identical on machines without
// the fonts installed.
import '@fontsource-variable/geist'
import '@fontsource-variable/jetbrains-mono'
import '@fontsource-variable/cascadia-code'
import './app.css'

mount(App, { target: document.getElementById('app')! })
