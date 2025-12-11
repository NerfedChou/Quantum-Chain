import { createApp } from 'vue'
import { createRouter, createWebHashHistory } from 'vue-router'
import App from './App.vue'
import './style.css'

// Views
import Dashboard from './views/Dashboard.vue'
import Subsystems from './views/Subsystems.vue'
import SubsystemDetail from './views/SubsystemDetail.vue'
import Anomalies from './views/Anomalies.vue'
import Settings from './views/Settings.vue'

// Router
const routes = [
    { path: '/', name: 'dashboard', component: Dashboard },
    { path: '/subsystems', name: 'subsystems', component: Subsystems },
    { path: '/subsystems/:id', name: 'subsystem-detail', component: SubsystemDetail },
    { path: '/anomalies', name: 'anomalies', component: Anomalies },
    { path: '/settings', name: 'settings', component: Settings }
]

const router = createRouter({
    history: createWebHashHistory(),
    routes
})

// Create app
const app = createApp(App)
app.use(router)
app.mount('#app')
