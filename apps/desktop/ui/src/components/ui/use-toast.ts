"use client"

import * as React from "react"

import type { ToastActionElement, ToastProps } from "@/components/ui/toast"

const TOAST_LIMIT = 3
const TOAST_REMOVE_DELAY = 4000

type ToasterToast = ToastProps & {
  id: string
  title?: React.ReactNode
  description?: React.ReactNode
  action?: ToastActionElement
}

type State = {
  toasts: ToasterToast[]
}

type Action =
  | { type: "ADD_TOAST"; toast: ToasterToast }
  | { type: "UPDATE_TOAST"; toast: Partial<ToasterToast> & { id: string } }
  | { type: "DISMISS_TOAST"; toastId?: string }
  | { type: "REMOVE_TOAST"; toastId?: string }

const listeners = new Set<(state: State) => void>()
let memoryState: State = { toasts: [] }

function dispatch(action: Action) {
  memoryState = reducer(memoryState, action)
  for (const listener of listeners) {
    listener(memoryState)
  }
}

function reducer(state: State, action: Action): State {
  switch (action.type) {
    case "ADD_TOAST": {
      const nextToasts = [action.toast, ...state.toasts].slice(0, TOAST_LIMIT)
      return { toasts: nextToasts }
    }
    case "UPDATE_TOAST": {
      return {
        toasts: state.toasts.map((toast) =>
          toast.id === action.toast.id ? { ...toast, ...action.toast } : toast
        ),
      }
    }
    case "DISMISS_TOAST": {
      if (action.toastId) {
        return {
          toasts: state.toasts.map((toast) =>
            toast.id === action.toastId ? { ...toast, open: false } : toast
          ),
        }
      }
      return {
        toasts: state.toasts.map((toast) => ({ ...toast, open: false })),
      }
    }
    case "REMOVE_TOAST": {
      if (action.toastId) {
        return {
          toasts: state.toasts.filter((toast) => toast.id !== action.toastId),
        }
      }
      return { toasts: [] }
    }
    default:
      return state
  }
}

let count = 0

function genId() {
  count = (count + 1) % Number.MAX_SAFE_INTEGER
  return count.toString()
}

function addToast(props: ToastProps & { id?: string } & {
  title?: React.ReactNode
  description?: React.ReactNode
  action?: ToastActionElement
}) {
  const id = props.id ?? genId()

  const toast: ToasterToast = {
    id,
    duration: props.duration ?? TOAST_REMOVE_DELAY,
    ...props,
  }

  dispatch({ type: "ADD_TOAST", toast })

  return {
    id,
    dismiss: () => dispatch({ type: "DISMISS_TOAST", toastId: id }),
    update: (props: Partial<ToasterToast>) =>
      dispatch({ type: "UPDATE_TOAST", toast: { id, ...props } }),
  }
}

function useToast() {
  const [state, setState] = React.useState<State>(memoryState)

  React.useEffect(() => {
    listeners.add(setState)
    return () => {
      listeners.delete(setState)
    }
  }, [])

  return {
    ...state,
    toast: addToast,
    dismiss: (toastId?: string) => dispatch({ type: "DISMISS_TOAST", toastId }),
    remove: (toastId?: string) => dispatch({ type: "REMOVE_TOAST", toastId }),
  }
}

export { useToast, addToast as toast }
