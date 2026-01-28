<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, nextTick } from 'vue'
import { useRoute, useRouter } from 'vue-router'

const route = useRoute()
const router = useRouter()

const assistantId = computed(() => route.params.assistantId as string)
const topicId = computed(() => route.params.topicId as string)

const isLoading = ref(true)
const error = ref<string | null>(null)
const graphData = ref<{
  nodes: Array<{ id: string; label: string; node_type: string; weight: number; metadata: Record<string, any> }>
  edges: Array<{ source: string; target: string; relation_type: string; weight: number }>
} | null>(null)

const canvasRef = ref<HTMLCanvasElement | null>(null)
const containerRef = ref<HTMLDivElement | null>(null)
const scale = ref(1)
const offsetX = ref(0)
const offsetY = ref(0)
const isDragging = ref(false)
const dragStart = ref({ x: 0, y: 0 })
const selectedNode = ref<string | null>(null)
const nodePositions = ref<Map<string, { x: number; y: number; vx: number; vy: number }>>(new Map())

const nodeColors: Record<string, string> = {
  'message': '#3b82f6', 'entity': '#10b981', 'concept': '#8b5cf6',
  'topic': '#f59e0b', 'emotion': '#ef4444', 'default': '#6b7280'
}
const edgeColors: Record<string, string> = {
  'reply_to': '#60a5fa', 'mentions': '#34d399', 'related_to': '#a78bfa',
  'causes': '#fbbf24', 'default': '#9ca3af'
}

async function loadGraphData() {
  if (!assistantId.value || !topicId.value) {
    error.value = '缺少助手ID或话题ID'
    isLoading.value = false
    return
  }
  try {
    isLoading.value = true
    error.value = null
    const response = await fetch(`http://localhost:7894/graphs/${assistantId.value}/${topicId.value}/temporal`)
    if (!response.ok) throw new Error(`HTTP ${response.status}: ${response.statusText}`)
    const result = await response.json()
    if (!result.success) throw new Error(result.error || '获取图数据失败')
    
    // 转换后端数据格式为前端格式
    const data = result.data
    graphData.value = {
      nodes: data.nodes.map((n: any) => ({
        id: n.id,
        label: n.label || n.id,
        node_type: n.memory_type || 'default',
        weight: 1,
        metadata: { timestamp: n.timestamp, content_preview: n.content_preview }
      })),
      edges: data.edges.map((e: any) => ({
        source: e.from,
        target: e.to,
        relation_type: 'temporal',
        weight: e.weight
      }))
    }
    
    // 确保 canvas 尺寸正确后再初始化
    await nextTick()
    resizeCanvas()
    initializeNodePositions()
    runForceLayout()
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    isLoading.value = false
  }
}

function initializeNodePositions() {
  if (!graphData.value || !containerRef.value) return
  
  // 使用容器尺寸而不是 canvas 尺寸（canvas 可能还没初始化）
  const width = containerRef.value.clientWidth || 800
  const height = containerRef.value.clientHeight || 600
  const centerX = width / 2
  const centerY = height / 2
  const radius = Math.min(width, height) / 3
  
  nodePositions.value.clear()
  graphData.value.nodes.forEach((node, index) => {
    const angle = (2 * Math.PI * index) / graphData.value!.nodes.length
    nodePositions.value.set(node.id, {
      x: centerX + radius * Math.cos(angle) + (Math.random() - 0.5) * 50,
      y: centerY + radius * Math.sin(angle) + (Math.random() - 0.5) * 50,
      vx: 0, vy: 0
    })
  })
}

function runForceLayout() {
  if (!graphData.value) return
  for (let i = 0; i < 100; i++) {
    graphData.value.nodes.forEach((node1) => {
      const pos1 = nodePositions.value.get(node1.id)!
      graphData.value!.nodes.forEach((node2) => {
        if (node1.id === node2.id) return
        const pos2 = nodePositions.value.get(node2.id)!
        const dx = pos1.x - pos2.x, dy = pos1.y - pos2.y
        const dist = Math.sqrt(dx * dx + dy * dy) || 1
        const force = 5000 / (dist * dist)
        pos1.vx += (dx / dist) * force
        pos1.vy += (dy / dist) * force
      })
    })
    graphData.value.edges.forEach((edge) => {
      const pos1 = nodePositions.value.get(edge.source)
      const pos2 = nodePositions.value.get(edge.target)
      if (!pos1 || !pos2) return
      const dx = pos2.x - pos1.x, dy = pos2.y - pos1.y
      const force = Math.sqrt(dx * dx + dy * dy) * 0.01
      pos1.vx += dx * force; pos1.vy += dy * force
      pos2.vx -= dx * force; pos2.vy -= dy * force
    })
    graphData.value.nodes.forEach((node) => {
      const pos = nodePositions.value.get(node.id)!
      pos.x += pos.vx; pos.y += pos.vy
      pos.vx *= 0.9; pos.vy *= 0.9
    })
  }
  centerGraph()
  draw()
}

function centerGraph() {
  if (!graphData.value || !containerRef.value) return
  
  const width = containerRef.value.clientWidth || 800
  const height = containerRef.value.clientHeight || 600
  
  let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity
  nodePositions.value.forEach((pos) => {
    minX = Math.min(minX, pos.x); maxX = Math.max(maxX, pos.x)
    minY = Math.min(minY, pos.y); maxY = Math.max(maxY, pos.y)
  })
  const dx = width / 2 - (minX + maxX) / 2
  const dy = height / 2 - (minY + maxY) / 2
  nodePositions.value.forEach((pos) => { pos.x += dx; pos.y += dy })
}

function draw() {
  if (!canvasRef.value || !graphData.value) return
  const canvas = canvasRef.value
  
  // 确保 canvas 有尺寸
  if (canvas.width === 0 || canvas.height === 0) {
    if (containerRef.value) {
      canvas.width = containerRef.value.clientWidth || 800
      canvas.height = containerRef.value.clientHeight || 600
    }
  }
  
  const ctx = canvas.getContext('2d')!
  ctx.fillStyle = '#1a1a2e'
  ctx.fillRect(0, 0, canvas.width, canvas.height)
  ctx.save()
  ctx.translate(offsetX.value, offsetY.value)
  ctx.scale(scale.value, scale.value)

  graphData.value.edges.forEach((edge) => {
    const pos1 = nodePositions.value.get(edge.source)
    const pos2 = nodePositions.value.get(edge.target)
    if (!pos1 || !pos2) return
    ctx.beginPath()
    ctx.moveTo(pos1.x, pos1.y)
    ctx.lineTo(pos2.x, pos2.y)
    ctx.strokeStyle = edgeColors[edge.relation_type] || edgeColors.default
    ctx.lineWidth = Math.max(1, edge.weight * 2)
    ctx.globalAlpha = 0.6
    ctx.stroke()
    ctx.globalAlpha = 1
  })

  graphData.value.nodes.forEach((node) => {
    const pos = nodePositions.value.get(node.id)
    if (!pos) return
    const radius = 20 + node.weight * 5
    ctx.beginPath()
    ctx.arc(pos.x, pos.y, radius, 0, Math.PI * 2)
    ctx.fillStyle = nodeColors[node.node_type] || nodeColors.default
    ctx.fill()
    if (selectedNode.value === node.id) {
      ctx.strokeStyle = '#ffffff'
      ctx.lineWidth = 3
      ctx.stroke()
    }
    ctx.fillStyle = '#ffffff'
    ctx.font = '12px sans-serif'
    ctx.textAlign = 'center'
    const label = node.label.length > 15 ? node.label.slice(0, 15) + '...' : node.label
    ctx.fillText(label, pos.x, pos.y + radius + 15)
  })
  ctx.restore()
}

function handleMouseDown(e: MouseEvent) {
  isDragging.value = true
  dragStart.value = { x: e.clientX - offsetX.value, y: e.clientY - offsetY.value }
}
function handleMouseMove(e: MouseEvent) {
  if (isDragging.value) {
    offsetX.value = e.clientX - dragStart.value.x
    offsetY.value = e.clientY - dragStart.value.y
    draw()
  }
}
function handleMouseUp() { isDragging.value = false }
function handleWheel(e: WheelEvent) {
  e.preventDefault()
  scale.value = Math.max(0.1, Math.min(5, scale.value * (e.deltaY > 0 ? 0.9 : 1.1)))
  draw()
}
function handleClick(e: MouseEvent) {
  if (!canvasRef.value || !graphData.value) return
  const rect = canvasRef.value.getBoundingClientRect()
  const x = (e.clientX - rect.left - offsetX.value) / scale.value
  const y = (e.clientY - rect.top - offsetY.value) / scale.value
  selectedNode.value = null
  for (const node of graphData.value.nodes) {
    const pos = nodePositions.value.get(node.id)
    if (!pos) continue
    if (Math.sqrt((x - pos.x) ** 2 + (y - pos.y) ** 2) <= 20 + node.weight * 5) {
      selectedNode.value = node.id
      break
    }
  }
  draw()
}

function resizeCanvas() {
  if (!canvasRef.value || !containerRef.value) return
  canvasRef.value.width = containerRef.value.clientWidth
  canvasRef.value.height = containerRef.value.clientHeight
  if (graphData.value) draw()
}

function goBack() { router.push(`/chat/${assistantId.value}/${topicId.value}`) }
function relayout() { initializeNodePositions(); runForceLayout() }
function resetView() { 
  scale.value = 1
  offsetX.value = 0
  offsetY.value = 0
  centerGraph()
  draw() 
}

// 重建关系图
async function rebuildGraph() {
  try {
    isLoading.value = true
    error.value = null
    const response = await fetch(`http://localhost:7894/graphs/${assistantId.value}/${topicId.value}/temporal/rebuild`, {
      method: 'POST'
    })
    if (!response.ok) throw new Error(`HTTP ${response.status}: ${response.statusText}`)
    const result = await response.json()
    if (!result.success) throw new Error(result.error || '重建失败')
    
    // 重新加载数据
    await loadGraphData()
  } catch (e) {
    error.value = (e as Error).message
    isLoading.value = false
  }
}

onMounted(() => { loadGraphData(); window.addEventListener('resize', resizeCanvas); setTimeout(resizeCanvas, 100) })
onUnmounted(() => { window.removeEventListener('resize', resizeCanvas) })
</script>

<template>
  <div class="flex flex-col h-full bg-dark-900">
    <header class="flex items-center justify-between px-6 py-4 border-b border-dark-700">
      <div class="flex items-center gap-4">
        <button @click="goBack" class="px-3 py-1.5 bg-dark-700 hover:bg-dark-600 rounded text-sm">← 返回对话</button>
        <h1 class="text-lg font-semibold">🔗 对话关系图</h1>
      </div>
      <div class="flex items-center gap-3">
        <button @click="rebuildGraph" class="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded text-sm">🔨 重建关系</button>
        <button @click="relayout" class="px-3 py-1.5 bg-dark-700 hover:bg-dark-600 rounded text-sm">🔄 重新布局</button>
        <button @click="resetView" class="px-3 py-1.5 bg-dark-700 hover:bg-dark-600 rounded text-sm">🎯 重置视图</button>
        <div class="text-sm text-dark-400">缩放: {{ (scale * 100).toFixed(0) }}%</div>
      </div>
    </header>
    <div class="flex-1 flex">
      <div ref="containerRef" class="flex-1 relative">
        <div v-if="isLoading" class="absolute inset-0 flex items-center justify-center">
          <div class="text-center"><div class="text-4xl mb-4 animate-pulse">🔄</div><p class="text-dark-400">加载关系图...</p></div>
        </div>
        <div v-else-if="error" class="absolute inset-0 flex items-center justify-center">
          <div class="text-center"><div class="text-4xl mb-4">❌</div><p class="text-red-400 mb-4">{{ error }}</p>
            <button @click="loadGraphData" class="px-4 py-2 bg-primary-600 hover:bg-primary-500 rounded">重试</button></div>
        </div>
        <div v-else-if="graphData && graphData.nodes.length === 0" class="absolute inset-0 flex items-center justify-center">
          <div class="text-center"><div class="text-4xl mb-4">📭</div><p class="text-dark-400">暂无关系图数据</p></div>
        </div>
        <canvas v-show="!isLoading && !error && graphData && graphData.nodes.length > 0" ref="canvasRef"
          class="w-full h-full cursor-grab" :class="{ 'cursor-grabbing': isDragging }"
          @mousedown="handleMouseDown" @mousemove="handleMouseMove" @mouseup="handleMouseUp"
          @mouseleave="handleMouseUp" @wheel="handleWheel" @click="handleClick" />
      </div>
      <div v-if="selectedNode && graphData" class="w-80 border-l border-dark-700 p-4 overflow-y-auto">
        <h3 class="text-sm font-semibold text-dark-300 mb-4">节点详情</h3>
        <template v-for="node in graphData.nodes" :key="node.id">
          <div v-if="node.id === selectedNode" class="space-y-3">
            <div><div class="text-xs text-dark-500">ID</div><div class="text-sm font-mono">{{ node.id }}</div></div>
            <div><div class="text-xs text-dark-500">标签</div><div class="text-sm">{{ node.label }}</div></div>
            <div><div class="text-xs text-dark-500">类型</div><span class="px-2 py-0.5 rounded text-xs" :style="{ backgroundColor: nodeColors[node.node_type] || nodeColors.default }">{{ node.node_type }}</span></div>
            <div><div class="text-xs text-dark-500">权重</div><div class="text-sm">{{ node.weight.toFixed(2) }}</div></div>
          </div>
        </template>
      </div>
    </div>
    <footer v-if="graphData" class="px-6 py-2 border-t border-dark-700 text-sm text-dark-400">
      节点: {{ graphData.nodes.length }} | 边: {{ graphData.edges.length }}
    </footer>
  </div>
</template>