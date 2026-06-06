//! App state — one tab per DynamoDB table.

use crate::config::{Config, Tab};
use crate::dynamodb::{self, DynamoEvent, Item, TableMeta};
use anyhow::Result;
use std::sync::mpsc::Receiver;

#[derive(Debug, Clone)]
pub struct TabSpec {
    pub region: Option<String>,
    pub table: String,
    pub scan_limit: u32,
}

impl TabSpec {
    pub fn resolve(t: &Tab, default_region: Option<&str>) -> Result<Self> {
        let region = t
            .region
            .clone()
            .or_else(|| default_region.map(str::to_string));
        Ok(Self {
            region,
            table: t.table.clone(),
            scan_limit: t.scan_limit,
        })
    }
}

pub struct TabState {
    pub name: String,
    pub spec: TabSpec,
    pub items: Vec<Item>,
    pub meta: Option<TableMeta>,
    pub selected: usize,
    pub last_error: Option<String>,
    pub loading: bool,
    pub pending: Option<Receiver<DynamoEvent>>,
}

impl TabState {
    fn empty(name: String, spec: TabSpec) -> Self {
        Self {
            name,
            spec,
            items: Vec::new(),
            meta: None,
            selected: 0,
            last_error: None,
            loading: false,
            pending: None,
        }
    }
}

pub struct App {
    pub cfg: Config,
    pub tabs: Vec<TabState>,
    pub active_tab: usize,
    pub status: String,
}

impl App {
    pub fn new(cfg: Config) -> Result<Self> {
        let mut tabs = Vec::with_capacity(cfg.tabs.len());
        for t in &cfg.tabs {
            let spec = TabSpec::resolve(t, cfg.region.as_deref())?;
            tabs.push(TabState::empty(t.name.clone(), spec));
        }
        let mut app = App {
            cfg,
            tabs,
            active_tab: 0,
            status: String::new(),
        };
        app.refresh_active();
        Ok(app)
    }

    pub fn active(&self) -> &TabState {
        &self.tabs[self.active_tab]
    }
    pub fn active_mut(&mut self) -> &mut TabState {
        &mut self.tabs[self.active_tab]
    }

    pub fn switch_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.active_tab = idx;
            // Only fetch on first activation; subsequent switches
            // reuse the cached items until the user hits `r`.
            if self.tabs[idx].items.is_empty() && !self.tabs[idx].loading {
                self.refresh_active();
            }
        }
    }

    pub fn move_selection(&mut self, delta: isize) {
        let tab = self.active_mut();
        if tab.items.is_empty() {
            return;
        }
        let n = tab.items.len() as isize;
        tab.selected = ((tab.selected as isize + delta).clamp(0, n - 1)) as usize;
    }

    pub fn refresh_active(&mut self) {
        let idx = self.active_tab;
        let spec = self.tabs[idx].spec.clone();
        let name = self.tabs[idx].name.clone();
        self.status = format!("scanning {name}…");
        let rx = dynamodb::spawn_scan(spec.table.clone(), spec.scan_limit, spec.region.clone());
        let t = &mut self.tabs[idx];
        t.loading = true;
        t.last_error = None;
        t.pending = Some(rx);
    }

    pub fn drain(&mut self) -> bool {
        let mut any = false;
        for tab in self.tabs.iter_mut() {
            let Some(rx) = tab.pending.take() else { continue };
            let mut done = false;
            loop {
                match rx.try_recv() {
                    Ok(DynamoEvent::Scanned { items, meta }) => {
                        any = true;
                        let n = items.len();
                        tab.items = items;
                        tab.meta = Some(meta);
                        tab.loading = false;
                        tab.last_error = None;
                        if tab.selected >= tab.items.len() {
                            tab.selected = tab.items.len().saturating_sub(1);
                        }
                        done = true;
                        self.status = format!("{} · {n} items (scan)", tab.name);
                    }
                    Ok(DynamoEvent::Failed(e)) => {
                        any = true;
                        tab.last_error = Some(e.clone());
                        tab.loading = false;
                        done = true;
                        self.status = format!("error: {e}");
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        done = true;
                        break;
                    }
                }
            }
            if !done {
                tab.pending = Some(rx);
            }
        }
        any
    }

    /// `o` — open the DynamoDB console for the active tab's table.
    pub fn open_console(&mut self) {
        let tab = self.active();
        let url = dynamodb::console_url(&tab.spec.table, tab.spec.region.as_deref());
        match webbrowser::open(&url) {
            Ok(()) => self.status = format!("opened {url}"),
            Err(e) => self.status = format!("open failed: {e}"),
        }
    }

    /// `y` — yank the focused item's JSON to OS clipboard.
    pub fn yank_focused_json(&mut self) {
        let tab = self.active();
        let Some(item) = tab.items.get(tab.selected) else {
            self.status = "no item selected".into();
            return;
        };
        let json = serde_json::to_string_pretty(&item.raw).unwrap_or_default();
        match crate::clipboard_copy(&json) {
            Ok(()) => self.status = format!("copied 1 item ({} bytes JSON)", json.len()),
            Err(e) => self.status = format!("copy failed: {e}"),
        }
    }
}
