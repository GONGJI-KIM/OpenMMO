import { mount } from 'svelte'
import './app.css'
import './lib/utils/assetUrl'
import App from './App.svelte'
import { registerModelServiceWorker } from './lib/utils/modelServiceWorker'

if (import.meta.env.PROD) void registerModelServiceWorker()

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
