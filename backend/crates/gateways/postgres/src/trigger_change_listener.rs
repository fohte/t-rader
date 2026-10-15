use sqlx::postgres::{PgListener, PgPool};

const TRIGGER_CHANGE_CHANNEL: &str = "t_rader_trigger_changed";

pub struct PostgresTriggerChangeListener {
    listener: PgListener,
}

impl PostgresTriggerChangeListener {
    pub async fn connect(pool: &PgPool) -> Result<Self, sqlx::Error> {
        let mut listener = PgListener::connect_with(pool).await?;
        listener.listen(TRIGGER_CHANGE_CHANNEL).await?;
        Ok(Self { listener })
    }

    pub async fn recv(&mut self) -> Result<(), sqlx::Error> {
        self.listener.try_recv().await?;
        Ok(())
    }
}
