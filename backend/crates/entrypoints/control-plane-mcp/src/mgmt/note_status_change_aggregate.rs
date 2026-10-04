use core_application::note_status_change_aggregate::{
    NoteStatusChangeAggregateUseCaseError, NoteStatusChangePeriod,
};
use rmcp::ErrorData as McpError;

use super::dto::{GetNoteStatusChangeCountsParams, GetNoteStatusChangeCountsResult};
use super::{MgmtServer, invalid_params, map_persistence_error};

impl MgmtServer {
    pub(super) async fn get_note_status_change_counts_inner(
        &self,
        params: GetNoteStatusChangeCountsParams,
    ) -> Result<GetNoteStatusChangeCountsResult, McpError> {
        let from = params.from;
        let to = params.to;
        let period = NoteStatusChangePeriod { from, to };
        let counts = self
            .dependencies
            .note_status_change_aggregate
            .count(period)
            .await
            .map_err(|error| {
                match error {
                NoteStatusChangeAggregateUseCaseError::InvalidPeriod => {
                    invalid_params("to must be later than from")
                }
                NoteStatusChangeAggregateUseCaseError::Query(
                    core_application::note_status_change_aggregate::
                        NoteStatusChangeAggregateQueryError::Database(error),
                ) => map_persistence_error(error),
            }
            })?;

        Ok(GetNoteStatusChangeCountsResult {
            from,
            to,
            approved_count: counts.approved,
            rejected_count: counts.rejected,
        })
    }
}
