import { createRouter, createWebHistory } from 'vue-router'
import DashboardView from '../views/DashboardView.vue'
import LoginView from '../views/LoginView.vue'

// 检查用户是否已认证
function isAuthenticated() {
  const authStatus = localStorage.getItem('hajimi_authenticated')
  const authTime = localStorage.getItem('hajimi_auth_time')
  
  if (!authStatus || authStatus !== 'true') {
    return false
  }
  
  // 检查认证是否过期（24小时）
  if (authTime) {
    const now = new Date().getTime()
    const authTimeStamp = parseInt(authTime)
    const hoursPassed = (now - authTimeStamp) / (1000 * 60 * 60)
    
    if (hoursPassed > 24) {
      // 认证过期，清除状态
      localStorage.removeItem('hajimi_authenticated')
      localStorage.removeItem('hajimi_auth_time')
      return false
    }
  }
  
  return true
}

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/login',
      name: 'login',
      component: LoginView,
    },
    {
      path: '/',
      name: 'dashboard',
      component: DashboardView,
      meta: { requiresAuth: true },
    },
    // 重定向所有其他路径到仪表盘
    {
      path: '/:pathMatch(.*)*',
      redirect: '/'
    }
  ],
})

// 全局路由守卫
router.beforeEach((to, from, next) => {
  // 如果路由需要认证
  if (to.meta.requiresAuth) {
    if (isAuthenticated()) {
      next() // 已认证，继续
    } else {
      next('/login') // 未认证，跳转到登录页
    }
  } else if (to.path === '/login' && isAuthenticated()) {
    next('/') // 已认证用户访问登录页，跳转到首页
  } else {
    next() // 不需要认证或访问登录页
  }
})

export default router
