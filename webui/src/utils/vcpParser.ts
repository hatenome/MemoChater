/**
 * VCP协议解析器
 * 用于解析消息中的工具调用块和工具返回块
 */

import { TOOL_START, TOOL_END, RESULT_START, RESULT_END, PARAM_START, PARAM_END } from '../config/vcp'

export interface VCPSegment {
  type: 'text' | 'tool_call' | 'tool_call_pending' | 'tool_result' | 'tool_result_pending'
  content: string
  parsed?: ToolCallParsed | ToolResultParsed
}

export interface ToolCallParsed {
  maid: string | null
  toolName: string | null
  command: string | null
  params: Record<string, string>
}

export interface ToolResultParsed {
  toolName: string | null
  status: 'success' | 'error' | null
  content: string | null
  parsedContent: unknown | null
}

/**
 * 解析消息内容，识别工具块和普通文本
 */
export function parseVCPContent(content: string): VCPSegment[] {
  if (!content) return [{ type: 'text', content: '' }]
  
  const segments: VCPSegment[] = []
  let remaining = content
  
  while (remaining.length > 0) {
    const toolCallStart = remaining.indexOf(TOOL_START)
    const resultStart = remaining.indexOf(RESULT_START)
    
    let nextMarkerPos = -1
    let markerType: 'tool_call' | 'tool_result' | null = null
    
    if (toolCallStart !== -1 && (resultStart === -1 || toolCallStart < resultStart)) {
      nextMarkerPos = toolCallStart
      markerType = 'tool_call'
    } else if (resultStart !== -1) {
      nextMarkerPos = resultStart
      markerType = 'tool_result'
    }
    
    if (nextMarkerPos === -1) {
      if (remaining.trim()) {
        segments.push({ type: 'text', content: remaining })
      }
      break
    }
    
    if (nextMarkerPos > 0) {
      const textBefore = remaining.substring(0, nextMarkerPos)
      if (textBefore.trim()) {
        segments.push({ type: 'text', content: textBefore })
      }
    }
    
    if (markerType === 'tool_call') {
      const endPos = remaining.indexOf(TOOL_END, nextMarkerPos)
      if (endPos !== -1) {
        const blockContent = remaining.substring(
          nextMarkerPos + TOOL_START.length,
          endPos
        )
        segments.push({
          type: 'tool_call',
          content: blockContent.trim(),
          parsed: parseToolCall(blockContent)
        })
        remaining = remaining.substring(endPos + TOOL_END.length)
      } else {
        segments.push({
          type: 'tool_call_pending',
          content: remaining.substring(nextMarkerPos + TOOL_START.length).trim()
        })
        break
      }
    } else if (markerType === 'tool_result') {
      // 查找配对的 ]]，需要处理嵌套情况
      const endPos = findResultEnd(remaining, nextMarkerPos + RESULT_START.length)
      if (endPos !== -1) {
        const blockContent = remaining.substring(
          nextMarkerPos + RESULT_START.length,
          endPos
        )
        segments.push({
          type: 'tool_result',
          content: blockContent.trim(),
          parsed: parseToolResult(blockContent)
        })
        remaining = remaining.substring(endPos + RESULT_END.length)
      } else {
        segments.push({
          type: 'tool_result_pending',
          content: remaining.substring(nextMarkerPos + RESULT_START.length).trim()
        })
        break
      }
    }
  }
  
  return segments.length > 0 ? segments : [{ type: 'text', content }]
}

/**
 * 查找工具返回块的结束位置，处理嵌套的 [[ ]]
 */
function findResultEnd(content: string, startPos: number): number {
  let depth = 1
  let pos = startPos
  
  while (pos < content.length && depth > 0) {
    const nextOpen = content.indexOf('[[', pos)
    const nextClose = content.indexOf(']]', pos)
    
    if (nextClose === -1) return -1
    
    if (nextOpen !== -1 && nextOpen < nextClose) {
      depth++
      pos = nextOpen + 2
    } else {
      depth--
      if (depth === 0) return nextClose
      pos = nextClose + 2
    }
  }
  
  return -1
}

/**
 * 解析工具调用内容
 */
function parseToolCall(content: string): ToolCallParsed {
  const result: ToolCallParsed = {
    maid: null,
    toolName: null,
    command: null,
    params: {}
  }
  
  const lines = content.split('\n').map(l => l.trim()).filter(l => l)
  const paramStart = escapeRegex(PARAM_START)
  const paramEnd = escapeRegex(PARAM_END)
  
  for (const line of lines) {
    const regex = new RegExp(`^(\\w+):${paramStart}(.*)${paramEnd},?$`)
    const match = line.match(regex)
    
    if (match) {
      const key = match[1]
      const value = match[2]
      if (key === 'maid') {
        result.maid = value
      } else if (key === 'tool_name') {
        result.toolName = value
      } else if (key === 'command') {
        result.command = value
      } else {
        result.params[key] = value
      }
    }
  }
  
  return result
}

function escapeRegex(str: string): string {
  return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

/**
 * 解析工具返回内容
 */
function parseToolResult(content: string): ToolResultParsed {
  const result: ToolResultParsed = {
    toolName: null,
    status: null,
    content: null,
    parsedContent: null
  }
  
  const lines = content.split('\n')
  let inContent = false
  const contentLines: string[] = []
  
  for (const line of lines) {
    if (line.includes('工具名称:')) {
      result.toolName = line.split('工具名称:')[1]?.trim() || null
    } else if (line.includes('执行状态:')) {
      const statusText = line.split('执行状态:')[1]?.trim() || ''
      result.status = statusText.includes('SUCCESS') ? 'success' : 'error'
    } else if (line.includes('返回内容:')) {
      inContent = true
      const firstContent = line.split('返回内容:')[1]?.trim()
      if (firstContent) contentLines.push(firstContent)
    } else if (inContent) {
      contentLines.push(line)
    }
  }
  
  result.content = contentLines.join('\n').trim()
  
  // 移除末尾的 "VCP调用结果结束"
  if (result.content?.endsWith('VCP调用结果结束')) {
    result.content = result.content.slice(0, -'VCP调用结果结束'.length).trim()
  }
  
  if (result.content) {
    try {
      result.parsedContent = JSON.parse(result.content)
    } catch {
      result.parsedContent = null
    }
  }
  
  return result
}

/**
 * 检查内容是否包含VCP工具块
 */
export function hasVCPBlocks(content: string): boolean {
  if (!content) return false
  return content.includes(TOOL_START) || 
         content.includes(RESULT_START)
}