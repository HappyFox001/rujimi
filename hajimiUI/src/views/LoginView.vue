<script setup>
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'

const router = useRouter()
const password = ref('')
const isLoading = ref(false)
const errorMessage = ref('')
const showPassword = ref(false)
const logoAnimated = ref(false)

// 页面加载动画
onMounted(() => {
  setTimeout(() => {
    logoAnimated.value = true
  }, 100)
})

// 登录验证
async function handleLogin() {
  if (!password.value.trim()) {
    errorMessage.value = '请输入密码'
    return
  }
  
  isLoading.value = true
  errorMessage.value = ''
  
  try {
    // 验证密码 - 调用后端验证接口
    const response = await fetch('/api/auth/login', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({ password: password.value })
    })
    
    if (response.ok) {
      // 登录成功，保存认证状态到 localStorage
      localStorage.setItem('hajimi_authenticated', 'true')
      localStorage.setItem('hajimi_auth_time', new Date().getTime().toString())
      
      // 跳转到仪表盘
      router.push('/')
    } else {
      const error = await response.json()
      errorMessage.value = error.message || '密码错误，请重试'
    }
  } catch (error) {
    console.error('登录错误:', error)
    errorMessage.value = '网络连接错误，请稍后重试'
  } finally {
    isLoading.value = false
  }
}

// 切换密码显示
function togglePassword() {
  showPassword.value = !showPassword.value
}

// 回车登录
function handleKeyup(event) {
  if (event.key === 'Enter') {
    handleLogin()
  }
}
</script>

<template>
  <div class="login-container">
    <!-- 背景装饰 -->
    <div class="background-decoration">
      <div class="circle circle-1"></div>
      <div class="circle circle-2"></div>
      <div class="circle circle-3"></div>
    </div>
    
    <!-- 登录表单 -->
    <div class="login-card" :class="{ 'animated': logoAnimated }">
      <!-- Logo和标题 -->
      <div class="logo-section">
        <div class="logo" :class="{ 'bounce': logoAnimated }">
          <div class="logo-icon">🤖</div>
          <h1 class="logo-text">Hajimi</h1>
        </div>
        <p class="subtitle">Gemini API 代理服务</p>
      </div>
      
      <!-- 登录表单 -->
      <form class="login-form" @submit.prevent="handleLogin">
        <div class="form-group">
          <label for="password" class="form-label">访问密码</label>
          <div class="password-input-wrapper">
            <input
              id="password"
              :type="showPassword ? 'text' : 'password'"
              v-model="password"
              class="form-input"
              placeholder="请输入访问密码"
              @keyup="handleKeyup"
              :disabled="isLoading"
            />
            <button
              type="button"
              class="password-toggle"
              @click="togglePassword"
              :disabled="isLoading"
            >
              {{ showPassword ? '🙈' : '👁️' }}
            </button>
          </div>
        </div>
        
        <!-- 错误信息 -->
        <div v-if="errorMessage" class="error-message">
          {{ errorMessage }}
        </div>
        
        <!-- 登录按钮 -->
        <button
          type="submit"
          class="login-button"
          :disabled="isLoading || !password.trim()"
          :class="{ 'loading': isLoading }"
        >
          <span v-if="!isLoading">🚀 进入 Hajimi</span>
          <span v-else class="loading-spinner">
            <div class="spinner"></div>
            验证中...
          </span>
        </button>
      </form>
      
      <!-- 提示信息 -->
      <div class="login-footer">
        <p class="tip">💡 请输入环境变量中配置的 PASSWORD 密码</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.login-container {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  padding: 20px;
  position: relative;
  overflow: hidden;
}

/* 背景装饰 */
.background-decoration {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 0;
}

.circle {
  position: absolute;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.1);
  animation: float 6s ease-in-out infinite;
}

.circle-1 {
  width: 200px;
  height: 200px;
  top: 10%;
  left: 10%;
  animation-delay: 0s;
}

.circle-2 {
  width: 150px;
  height: 150px;
  top: 60%;
  right: 15%;
  animation-delay: 2s;
}

.circle-3 {
  width: 100px;
  height: 100px;
  bottom: 20%;
  left: 20%;
  animation-delay: 4s;
}

@keyframes float {
  0%, 100% { transform: translateY(0px) rotate(0deg); }
  50% { transform: translateY(-20px) rotate(180deg); }
}

/* 登录卡片 */
.login-card {
  background: rgba(255, 255, 255, 0.95);
  backdrop-filter: blur(15px);
  border-radius: 20px;
  padding: 40px;
  box-shadow: 0 20px 40px rgba(0, 0, 0, 0.2);
  width: 100%;
  max-width: 400px;
  position: relative;
  z-index: 1;
  transform: translateY(20px);
  opacity: 0;
  transition: all 0.5s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.login-card.animated {
  transform: translateY(0);
  opacity: 1;
}

/* Logo部分 */
.logo-section {
  text-align: center;
  margin-bottom: 30px;
}

.logo {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 15px;
  margin-bottom: 10px;
  transform: scale(0.8);
  opacity: 0;
  transition: all 0.6s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.logo.bounce {
  transform: scale(1);
  opacity: 1;
  animation: logoEntry 0.8s cubic-bezier(0.34, 1.56, 0.64, 1);
}

@keyframes logoEntry {
  0% { transform: scale(0.5) rotate(-10deg); opacity: 0; }
  50% { transform: scale(1.1) rotate(5deg); opacity: 0.8; }
  100% { transform: scale(1) rotate(0deg); opacity: 1; }
}

.logo-icon {
  font-size: 48px;
  background: linear-gradient(135deg, #667eea, #764ba2);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  filter: drop-shadow(0 4px 8px rgba(0, 0, 0, 0.2));
}

.logo-text {
  font-size: 36px;
  font-weight: 700;
  background: linear-gradient(135deg, #667eea, #764ba2);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  margin: 0;
  letter-spacing: -1px;
}

.subtitle {
  color: #666;
  font-size: 14px;
  margin: 0;
  opacity: 0.8;
}

/* 表单样式 */
.login-form {
  margin-bottom: 20px;
}

.form-group {
  margin-bottom: 20px;
}

.form-label {
  display: block;
  color: #333;
  font-size: 14px;
  font-weight: 500;
  margin-bottom: 8px;
}

.password-input-wrapper {
  position: relative;
}

.form-input {
  width: 100%;
  padding: 14px 50px 14px 16px;
  border: 2px solid #e1e5e9;
  border-radius: 12px;
  font-size: 16px;
  background: #fff;
  color: #333;
  transition: all 0.3s ease;
  box-sizing: border-box;
}

.form-input:focus {
  outline: none;
  border-color: #667eea;
  box-shadow: 0 0 0 3px rgba(102, 126, 234, 0.1);
  transform: translateY(-2px);
}

.form-input:disabled {
  background: #f5f5f5;
  cursor: not-allowed;
  opacity: 0.7;
}

.password-toggle {
  position: absolute;
  right: 12px;
  top: 50%;
  transform: translateY(-50%);
  background: none;
  border: none;
  font-size: 18px;
  cursor: pointer;
  padding: 6px;
  border-radius: 6px;
  transition: all 0.2s ease;
  opacity: 0.6;
}

.password-toggle:hover {
  opacity: 1;
  background: rgba(102, 126, 234, 0.1);
}

.password-toggle:disabled {
  cursor: not-allowed;
  opacity: 0.3;
}

/* 错误信息 */
.error-message {
  background: #fee;
  border: 1px solid #fcc;
  color: #c33;
  padding: 12px 16px;
  border-radius: 8px;
  font-size: 14px;
  margin-bottom: 20px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.error-message::before {
  content: '⚠️';
  font-size: 16px;
}

/* 登录按钮 */
.login-button {
  width: 100%;
  padding: 16px;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  border: none;
  border-radius: 12px;
  font-size: 16px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.3s ease;
  position: relative;
  overflow: hidden;
  box-shadow: 0 4px 15px rgba(102, 126, 234, 0.4);
}

.login-button:hover:not(:disabled) {
  transform: translateY(-2px);
  box-shadow: 0 8px 25px rgba(102, 126, 234, 0.5);
}

.login-button:active:not(:disabled) {
  transform: translateY(0);
}

.login-button:disabled {
  opacity: 0.7;
  cursor: not-allowed;
  transform: none;
}

.login-button.loading {
  pointer-events: none;
}

.loading-spinner {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
}

.spinner {
  width: 20px;
  height: 20px;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-top: 2px solid white;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}

/* 底部提示 */
.login-footer {
  text-align: center;
  padding-top: 20px;
  border-top: 1px solid #e1e5e9;
}

.tip {
  color: #666;
  font-size: 12px;
  margin: 0;
  opacity: 0.8;
}

/* 响应式设计 */
@media (max-width: 480px) {
  .login-container {
    padding: 15px;
  }
  
  .login-card {
    padding: 30px 20px;
  }
  
  .logo-icon {
    font-size: 40px;
  }
  
  .logo-text {
    font-size: 28px;
  }
  
  .form-input {
    font-size: 16px; /* 防止iOS缩放 */
  }
  
  .login-button {
    padding: 14px;
    font-size: 15px;
  }
}

/* 深色模式支持 */
@media (prefers-color-scheme: dark) {
  .login-card {
    background: rgba(30, 30, 30, 0.95);
    color: #fff;
  }
  
  .form-label {
    color: #ccc;
  }
  
  .form-input {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.2);
    color: #fff;
  }
  
  .form-input::placeholder {
    color: rgba(255, 255, 255, 0.6);
  }
  
  .form-input:focus {
    border-color: #667eea;
    background: rgba(255, 255, 255, 0.15);
  }
  
  .subtitle {
    color: #aaa;
  }
  
  .tip {
    color: #aaa;
  }
  
  .login-footer {
    border-top-color: rgba(255, 255, 255, 0.1);
  }
}
</style>