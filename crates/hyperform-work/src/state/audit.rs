//! Folding audit findings: `Audited` upserts by id, keeping the order a
//! finding was first found in; the user's add or dismiss sets its status.

use hyperform_protocol::work::{AuditFinding, AuditStatus};

use super::PlanState;

impl PlanState {
    pub(super) fn apply_audited(&mut self, findings: &[AuditFinding], rev: u64) {
        for finding in findings {
            let mut finding = finding.clone();
            finding.revision = rev;
            match self.audit.iter_mut().find(|old| old.id == finding.id) {
                Some(old) => {
                    finding.created_revision = old.created_revision;
                    *old = finding;
                }
                None => {
                    finding.created_revision = rev;
                    self.audit.push(finding);
                }
            }
        }
    }

    /// False when the finding is unknown.
    pub(super) fn set_finding_status(&mut self, id: &str, status: AuditStatus, rev: u64) -> bool {
        let Some(finding) = self.audit.iter_mut().find(|finding| finding.id == id) else {
            return false;
        };
        finding.status = status;
        finding.revision = rev;
        true
    }
}
