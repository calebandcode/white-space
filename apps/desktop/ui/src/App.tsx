import { HashRouter, Navigate, Route, Routes } from "react-router-dom"

import ApplicationLayout from "@/components/application-modal"
import { Home } from "@/pages/home"
import { Activity } from "@/pages/activity"
import { Archive } from "@/pages/archive"

function App() {
  return (
    <HashRouter>
      <ApplicationLayout>
        <Routes>
          <Route path="/" element={<Home />} />
          <Route path="/activity" element={<Activity />} />
          <Route path="/archive" element={<Archive />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </ApplicationLayout>
    </HashRouter>
  )
}

export default App
