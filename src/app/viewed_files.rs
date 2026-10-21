use super::*;

#[derive(Debug, Clone)]
pub(super) enum ViewedFilesState {
    Loading,
    Loaded(PullRequestFileViewedStates),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FileViewedUpdate {
    pub(super) item_id: String,
    pub(super) pull_request_id: String,
    pub(super) path: String,
    pub(super) viewed: bool,
}

pub(super) fn viewed_file_count(
    diff: &PullRequestDiff,
    viewed: &PullRequestFileViewedStates,
) -> usize {
    diff.files
        .iter()
        .filter(|file| viewed.files.get(&diff_display_path(file)) == Some(&FileViewedState::Viewed))
        .count()
}

impl AppState {
    pub(super) fn ensure_viewed_files_loading(
        &mut self,
        item: &WorkItem,
        tx: &UnboundedSender<AppMsg>,
    ) -> bool {
        if self.viewed_files.contains_key(&item.id) {
            return false;
        }
        self.viewed_files
            .insert(item.id.clone(), ViewedFilesState::Loading);
        start_viewed_files_load(item.clone(), tx.clone());
        true
    }

    pub(super) fn finish_viewed_files_load(
        &mut self,
        item_id: String,
        result: std::result::Result<PullRequestFileViewedStates, String>,
    ) {
        let state = match result {
            Ok(viewed) => ViewedFilesState::Loaded(viewed),
            Err(error) => ViewedFilesState::Error(error),
        };
        self.viewed_files.insert(item_id, state);
    }

    pub(super) fn forget_viewed_files_unless_loading(&mut self, item_id: &str) {
        if !matches!(
            self.viewed_files.get(item_id),
            Some(ViewedFilesState::Loading)
        ) {
            self.viewed_files.remove(item_id);
        }
    }

    pub(super) fn current_viewed_files(&self) -> Option<&PullRequestFileViewedStates> {
        let item = self.current_item()?;
        match self.viewed_files.get(&item.id) {
            Some(ViewedFilesState::Loaded(viewed)) => Some(viewed),
            _ => None,
        }
    }

    pub(super) fn toggle_selected_diff_file_viewed(&mut self, tx: &UnboundedSender<AppMsg>) {
        if let Some(update) = self.begin_selected_diff_file_viewed_toggle() {
            start_file_viewed_update(update, tx.clone());
        }
    }

    pub(super) fn begin_selected_diff_file_viewed_toggle(&mut self) -> Option<FileViewedUpdate> {
        let Some(item) = self.current_item().cloned() else {
            self.status = "nothing selected".to_string();
            return None;
        };
        let Some(DiffState::Loaded(diff)) = self.current_diff() else {
            self.status = "diff still loading".to_string();
            return None;
        };
        let Some(file) = diff
            .files
            .get(self.selected_diff_file_index_for(&item.id, diff))
        else {
            self.status = "no diff files".to_string();
            return None;
        };
        let path = diff_display_path(file);
        let viewed_files = match self.viewed_files.get(&item.id) {
            Some(ViewedFilesState::Loaded(viewed_files)) => viewed_files,
            Some(ViewedFilesState::Error(error)) => {
                self.status = format!(
                    "viewed state unavailable: {}; press r to retry",
                    operation_error_body(error)
                );
                return None;
            }
            Some(ViewedFilesState::Loading) | None => {
                self.status = "viewed state still loading".to_string();
                return None;
            }
        };
        let update_key = (item.id.clone(), path.clone());
        if self.viewed_file_updates.contains(&update_key) {
            self.status = format!("viewed update already running for {path}");
            return None;
        }

        let viewed = viewed_files.files.get(&path) != Some(&FileViewedState::Viewed);
        let pull_request_id = viewed_files.pull_request_id.clone();
        self.viewed_file_updates.insert(update_key);
        self.status = file_viewed_running_status(viewed, &path);
        Some(FileViewedUpdate {
            item_id: item.id,
            pull_request_id,
            path,
            viewed,
        })
    }

    pub(super) fn finish_file_viewed_update(
        &mut self,
        update: FileViewedUpdate,
        result: std::result::Result<(), String>,
    ) {
        self.viewed_file_updates
            .remove(&(update.item_id.clone(), update.path.clone()));
        match result {
            Ok(()) => {
                if let Some(ViewedFilesState::Loaded(viewed_files)) =
                    self.viewed_files.get_mut(&update.item_id)
                {
                    let confirmed = if update.viewed {
                        FileViewedState::Viewed
                    } else {
                        FileViewedState::Unviewed
                    };
                    viewed_files.files.insert(update.path.clone(), confirmed);
                }
                self.status = file_viewed_success_status(update.viewed, &update.path);
            }
            Err(error) => {
                self.show_operation_error_dialog("Viewed Update Failed", &error);
                self.status = format!(
                    "viewed update failed for {}: {}",
                    update.path,
                    operation_error_body(&error)
                );
            }
        }
    }
}

fn file_viewed_running_status(viewed: bool, path: &str) -> String {
    if viewed {
        format!("marking {path} as viewed")
    } else {
        format!("unmarking {path} as viewed")
    }
}

fn file_viewed_success_status(viewed: bool, path: &str) -> String {
    if viewed {
        format!("marked {path} as viewed")
    } else {
        format!("unmarked {path} as viewed")
    }
}
