//! A throwaway account for `smoke` and `status`: an invite, a confirmed user created through the Admin API with that
//! invite (the sign-up hook still runs), and a session for it. Everything is deleted afterwards, also when the work in
//! between fails. The service-role key lives only in this process.

use anyhow::Result;
use serde_json::json;

use crate::cmd::supabase::{create_invite, delete_invite};
use crate::secret::Secret;
use crate::supabase::{Management, Project, Session};
use crate::timefmt;

pub struct TestUser {
    pub project: Project,
    pub session: Session,
    pub email: String,
    mgmt: Management,
    service: Secret,
    invite: String,
}

impl TestUser {
    fn create(label: &str) -> Result<Self> {
        let mut mgmt = Management::from_env()?;
        let keys = mgmt.keys()?;
        let mut project = Project::new(&mgmt.project_url, keys.publishable.clone())?;
        project.redact(&keys.service_role);
        let invite = create_invite(&mgmt, 1, Some(&format!("xtask {label}")), None)?;
        // example.com never receives mail, and no mail is sent for Admin-created, pre-confirmed users anyway.
        let email = format!("xtask-{label}-{}@example.com", timefmt::now_unix());
        let created = project.admin_create_user(&keys.service_role, &email, None, &json!({ "invite_code": invite }));
        let user_id = match created {
            Ok(id) => id,
            Err(e) => {
                let _ = delete_invite(&mgmt, &invite);
                return Err(e);
            }
        };
        let session = match project.magic_link_session(&keys.service_role, &email) {
            Ok(s) => s,
            Err(e) => {
                let _ = project.admin_delete_user(&keys.service_role, &user_id);
                let _ = delete_invite(&mgmt, &invite);
                return Err(e);
            }
        };
        Ok(TestUser { project, session, email, mgmt, service: keys.service_role, invite })
    }

    fn delete(self) -> Result<()> {
        let user = self.project.admin_delete_user(&self.service, &self.session.user_id);
        let invite = delete_invite(&self.mgmt, &self.invite);
        user.and(invite)
    }
}

/// Run `work` as a fresh user, then delete the user and its invite whatever happened.
pub fn with_test_user<T>(label: &str, work: impl FnOnce(&TestUser) -> Result<T>) -> Result<T> {
    let user = TestUser::create(label)?;
    println!("test user {} created", user.session.user_id);
    let result = work(&user);
    let id = user.session.user_id.clone();
    match user.delete() {
        Ok(()) => println!("test user {id} deleted"),
        Err(e) => eprintln!("warning: could not delete test user {id}: {e:#}"),
    }
    result
}
