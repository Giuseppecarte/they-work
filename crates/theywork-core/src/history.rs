use std::collections::{BTreeMap, VecDeque};

use crate::{CollaborationEvent, Millis, OfficeId, COLLABORATION_HISTORY_LEN};

/// The observer's current project window. Evictions count retention actions,
/// not unique missing deliveries. Earlier provider history remains unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryWindow {
    pub retained_count: usize,
    pub oldest_retained_at: Option<Millis>,
    pub evicted_count: u64,
    pub last_eviction_ordinal: Option<u64>,
    pub last_evicted_record_at: Option<Millis>,
    pub prior_history_unknown: bool,
    pub prior_local_evictions_unknown: bool,
    last_observed_ordinal: u64,
}

impl Default for HistoryWindow {
    fn default() -> Self {
        Self {
            retained_count: 0,
            oldest_retained_at: None,
            evicted_count: 0,
            last_eviction_ordinal: None,
            last_evicted_record_at: None,
            prior_history_unknown: true,
            prior_local_evictions_unknown: false,
            last_observed_ordinal: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct Record {
    office: OfficeId,
    event: CollaborationEvent,
}

/// Fair bounded retention, including bounded metadata for evicted projects.
#[derive(Debug, Default, Clone)]
pub(crate) struct HistoryStore {
    records: VecDeque<Record>,
    windows: BTreeMap<OfficeId, HistoryWindow>,
    ordinal: u64,
    older_project_windows_unknown: bool,
}

impl HistoryStore {
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &CollaborationEvent> {
        self.records.iter().map(|record| &record.event)
    }

    pub fn office(&self, event: &CollaborationEvent) -> Option<&OfficeId> {
        self.records
            .iter()
            .find(|record| record.event.actor == event.actor && record.event.id == event.id)
            .map(|record| &record.office)
    }

    pub fn window(&self, office: &OfficeId) -> HistoryWindow {
        self.windows.get(office).cloned().unwrap_or(HistoryWindow {
            prior_local_evictions_unknown: self.older_project_windows_unknown,
            ..HistoryWindow::default()
        })
    }

    pub fn windows(&self) -> impl Iterator<Item = (&OfficeId, &HistoryWindow)> {
        self.windows.iter()
    }

    pub fn older_project_windows_unknown(&self) -> bool {
        self.older_project_windows_unknown
    }

    pub fn insert(&mut self, mut office: OfficeId, event: CollaborationEvent) {
        if let Some(index) = self
            .records
            .iter()
            .position(|record| record.event.actor == event.actor && record.event.id == event.id)
        {
            let old = &self.records[index];
            if old.event.at > event.at || old.event == event {
                return;
            }
            // The same record can be enriched after its actor moved. Its
            // original project is evidence, not the actor's current location.
            office = self
                .records
                .remove(index)
                .expect("known record index")
                .office;
        }
        self.ordinal = self.ordinal.saturating_add(1);
        self.windows
            .entry(office.clone())
            .or_insert_with(|| HistoryWindow {
                prior_local_evictions_unknown: self.older_project_windows_unknown,
                ..HistoryWindow::default()
            })
            .last_observed_ordinal = self.ordinal;
        let index = self
            .records
            .iter()
            .position(|record| {
                (record.event.at, &record.event.actor, &record.event.id)
                    > (event.at, &event.actor, &event.id)
            })
            .unwrap_or(self.records.len());
        self.records.insert(index, Record { office, event });
        self.recount();
        if self.records.len() > COLLABORATION_HISTORY_LEN {
            // Largest partition first; ties retire the oldest source record,
            // then the lexically first project. Source time never drives age
            // of project metadata: that uses the observer's ordinal below.
            let victim = self
                .windows
                .iter()
                .filter(|(_, window)| window.retained_count > 0)
                .min_by_key(|(office, window)| {
                    (
                        std::cmp::Reverse(window.retained_count),
                        window.oldest_retained_at,
                        *office,
                    )
                })
                .map(|(office, _)| office.clone())
                .expect("a full store has a project");
            let index = self
                .records
                .iter()
                .position(|record| record.office == victim)
                .expect("a populated project has a record");
            let retired = self.records.remove(index).expect("known record index");
            let window = self.windows.get_mut(&victim).expect("known project");
            window.evicted_count = window.evicted_count.saturating_add(1);
            window.last_eviction_ordinal = Some(self.ordinal);
            window.last_evicted_record_at = Some(retired.event.at);
            self.recount();
        }
        while self.windows.len() > COLLABORATION_HISTORY_LEN {
            let oldest = self
                .windows
                .iter()
                .filter(|(_, window)| window.retained_count == 0)
                .min_by_key(|(office, window)| (window.last_observed_ordinal, *office))
                .map(|(office, _)| office.clone())
                .expect("more project windows than records leaves an empty window");
            self.windows.remove(&oldest);
            self.older_project_windows_unknown = true;
        }
    }

    fn recount(&mut self) {
        for window in self.windows.values_mut() {
            window.retained_count = 0;
            window.oldest_retained_at = None;
        }
        for record in &self.records {
            let window = self
                .windows
                .get_mut(&record.office)
                .expect("record project");
            window.retained_count += 1;
            window.oldest_retained_at.get_or_insert(record.event.at);
        }
    }
}
