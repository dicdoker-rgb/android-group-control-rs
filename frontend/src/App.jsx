import { useState, useRef } from "react";
import AppProvider, { useApp } from "./context/AppContext";
import { Sidebar } from "./components/layout/Sidebar";
import { Header } from "./components/layout/Header";
import { ActionBar } from "./components/layout/ActionBar";
import { DevicesPage } from "./pages/DevicesPage";
import { ControlPage } from "./pages/ControlPage";
import { TasksPage } from "./pages/TasksPage";
import { ToolsPage } from "./pages/ToolsPage";
import { OpsPage } from "./pages/OpsPage";
import { SettingsPage } from "./pages/SettingsPage";
import { IS_TAURI_DESKTOP } from "./api";

function AppShell() {
  const {
    status,
    devices,
    selected,
    search,
    setSearch,
    filterMode,
    setFilterMode,
    refreshAll,
    refreshPreviews,
    selectAllOnline,
    clearSelection,
    runAction,
    previewTimeLabel,
    preferencesOpen,
    setPreferencesOpen,
  } = useApp();

  const [activePage, setActivePage] = useState("devices");
  const searchInputRef = useRef(null);

  function handleAction(action, params) {
    runAction(action, params);
  }

  return (
    <div className="app-shell">
      <Sidebar activePage={activePage} onPageChange={setActivePage} />

      <div className="page-content">
        <Header
          status={status}
          selected={selected}
          search={search}
          onSearchChange={setSearch}
          filterMode={filterMode}
          onFilterChange={setFilterMode}
          onRefresh={refreshAll}
          onRefreshPreviews={refreshPreviews}
          onSelectAll={selectAllOnline}
          onClearSelection={clearSelection}
          onOpenPreferences={() => setPreferencesOpen(true)}
          previewTimeLabel={previewTimeLabel || ""}
          isTauriDesktop={IS_TAURI_DESKTOP}
          searchInputRef={searchInputRef}
        />

        <main className="page-main">
          {activePage === "devices" && <DevicesPage />}
          {activePage === "control" && <ControlPage />}
          {activePage === "tasks" && <TasksPage />}
          {activePage === "tools" && <ToolsPage />}
          {activePage === "ops" && <OpsPage />}
          {activePage === "settings" && <SettingsPage />}
        </main>

        <ActionBar
          selected={selected}
          onAction={handleAction}
          onClearSelection={clearSelection}
          onSelectAll={selectAllOnline}
        />
      </div>
    </div>
  );
}

export default function App() {
  return (
    <AppProvider>
      <AppShell />
    </AppProvider>
  );
}
