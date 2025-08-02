
mod linux;
mod network1;

use anyhow::{bail, Context, Result};
use tracing::{error, info, warn};


#[cfg(test)]
mod tests {
    use super::*;
    use macro_rules_attribute::apply;
    use smol_macros::test;
    use tracing_test::traced_test;
    use zbus::Connection;

    #[apply(test!)]
    #[traced_test]
    async fn test_zbus_connect() -> Result<()> {
        let conn = Connection::system().await?;
        Ok(())
    }

}
