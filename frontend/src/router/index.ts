import { createRouter, createWebHashHistory } from 'vue-router'
import AboutView from '../views/AboutView.vue'
import HomeView from '../views/HomeView.vue'
import PublicSubmission from '../components/PublicSubmission.vue'

// Hash history: works on any static host (the app is served by the API server's
// ServeDir), so deep links like /#/s/<token> always resolve without server-side
// route config.
const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', component: AboutView },        // landing page
    { path: '/app', component: HomeView },      // the budget tool
    { path: '/s/:token', component: PublicSubmission, props: true },
  ],
})

export default router
