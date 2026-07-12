CREATE TABLE "a2a_push_configs" (
	"task_id" text NOT NULL,
	"config_id" text NOT NULL,
	"config" jsonb NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	CONSTRAINT "a2a_push_configs_task_id_config_id_pk" PRIMARY KEY("task_id","config_id")
);
--> statement-breakpoint
CREATE TABLE "a2a_tasks" (
	"task_id" text PRIMARY KEY NOT NULL,
	"context_id" text NOT NULL,
	"state" text NOT NULL,
	"status_timestamp" timestamp with time zone NOT NULL,
	"protocol_version" text DEFAULT '0.3' NOT NULL,
	"task" jsonb NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE INDEX "a2a_tasks_context_idx" ON "a2a_tasks" USING btree ("context_id");--> statement-breakpoint
CREATE INDEX "a2a_tasks_sweep_idx" ON "a2a_tasks" USING btree ("state","status_timestamp");
