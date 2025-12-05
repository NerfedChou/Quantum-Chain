//! Application state management.

use std::collections::HashMap;

use super::{SubsystemId, SubsystemInfo, SubsystemStatus, SystemHealth};

/// Application state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppState {
    /// Main dashboard view.
    #[default]
    Dashboard,
    /// Help overlay.
    Help,
    /// Quitting.
    Quit,
}

/// Peer information for display.
#[derive(Debug, Clone, Default)]
pub struct PeerDisplayInfo {
    pub node_id: String,
    pub ip_address: String,
    pub port: String,
    pub reputation: u8,
    pub last_seen: String,
}

/// Pending block assembly information for display (qc-02).
#[derive(Debug, Clone, Default)]
pub struct PendingAssemblyInfo {
    pub block_hash: String,
    pub has_block: bool,
    pub has_merkle: bool,
    pub has_state: bool,
    pub started_at: u64,
}

/// Main application model.
pub struct App {
    /// Current application state/view.
    pub state: AppState,
    /// Currently selected subsystem.
    pub selected_subsystem: SubsystemId,
    /// Information for each subsystem.
    pub subsystems: HashMap<SubsystemId, SubsystemInfo>,
    /// System health metrics.
    pub system_health: SystemHealth,
    /// Last refresh timestamp.
    pub last_refresh: Option<chrono::DateTime<chrono::Utc>>,
    /// Error message to display (if any).
    pub error_message: Option<String>,
    /// Connected peers list (for qc-01 panel).
    pub peers: Vec<PeerDisplayInfo>,
    /// Pending block assemblies (for qc-02 panel).
    pub pending_assemblies: Vec<PendingAssemblyInfo>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Create a new application instance.
    pub fn new() -> Self {
        let mut subsystems = HashMap::new();

        // Initialize all subsystems with default state
        for id in SubsystemId::ALL {
            let status = if id.is_implemented() {
                SubsystemStatus::Stopped // Will be updated by health check
            } else {
                SubsystemStatus::NotImplemented
            };

            subsystems.insert(
                id,
                SubsystemInfo {
                    id,
                    status,
                    warning_message: None,
                    metrics: None,
                },
            );
        }

        Self {
            state: AppState::Dashboard,
            selected_subsystem: SubsystemId::PeerDiscovery,
            subsystems,
            system_health: SystemHealth::default(),
            last_refresh: None,
            error_message: None,
            peers: Vec::new(),
            pending_assemblies: Vec::new(),
        }
    }

    /// Handle keyboard input.
    pub fn handle_key(&mut self, key: char) {
        match self.state {
            AppState::Dashboard => self.handle_dashboard_key(key),
            AppState::Help => {
                // Any key closes help
                self.state = AppState::Dashboard;
            }
            AppState::Quit => {}
        }
    }

    fn handle_dashboard_key(&mut self, key: char) {
        match key {
            'q' | 'Q' => self.state = AppState::Quit,
            '?' => self.state = AppState::Help,
            // Subsystem selection via hotkeys
            c if SubsystemId::from_hotkey(c).is_some() => {
                if let Some(id) = SubsystemId::from_hotkey(c) {
                    self.selected_subsystem = id;
                }
            }
            _ => {}
        }
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        let current_idx = SubsystemId::ALL
            .iter()
            .position(|&id| id == self.selected_subsystem)
            .unwrap_or(0);

        let new_idx = if current_idx == 0 {
            SubsystemId::ALL.len() - 1
        } else {
            current_idx - 1
        };

        self.selected_subsystem = SubsystemId::ALL[new_idx];
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        let current_idx = SubsystemId::ALL
            .iter()
            .position(|&id| id == self.selected_subsystem)
            .unwrap_or(0);

        let new_idx = (current_idx + 1) % SubsystemId::ALL.len();
        self.selected_subsystem = SubsystemId::ALL[new_idx];
    }

    /// Get the currently selected subsystem info.
    pub fn selected_info(&self) -> &SubsystemInfo {
        self.subsystems
            .get(&self.selected_subsystem)
            .expect("selected subsystem should always exist")
    }

    /// Update subsystem info.
    pub fn update_subsystem(&mut self, info: SubsystemInfo) {
        self.subsystems.insert(info.id, info);
    }

    /// Check if the app should quit.
    pub fn should_quit(&self) -> bool {
        self.state == AppState::Quit
    }
}
