package provider

import (
	"context"
	"errors"
	"fmt"
	"time"

	"github.com/hashicorp/terraform-plugin-framework/diag"
	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/stringplanmodifier"
	"github.com/hashicorp/terraform-plugin-framework/schema/validator"
	"github.com/hashicorp/terraform-plugin-framework/types"
	"github.com/oapi-codegen/nullable"
	openapi_types "github.com/oapi-codegen/runtime/types"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
	traderapigen "github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

var (
	_ resource.Resource                     = (*strategyTriggerResource)(nil)
	_ resource.ResourceWithImportState      = (*strategyTriggerResource)(nil)
	_ resource.ResourceWithConfigValidators = (*strategyTriggerResource)(nil)
)

type strategyTriggerResource struct {
	client *traderapi.Client
}

type strategyTriggerModel struct {
	ID             types.String  `tfsdk:"id"`
	StrategyID     types.String  `tfsdk:"strategy_id"`
	Purpose        types.String  `tfsdk:"purpose"`
	Kind           types.String  `tfsdk:"kind"`
	Schedule       types.String  `tfsdk:"schedule"`
	HookSlug       types.String  `tfsdk:"hook_slug"`
	EventMatch     types.Dynamic `tfsdk:"event_match"`
	PromptTemplate types.String  `tfsdk:"prompt_template"`
	Enabled        types.Bool    `tfsdk:"enabled"`
	CreatedAt      types.String  `tfsdk:"created_at"`
	UpdatedAt      types.String  `tfsdk:"updated_at"`
}

func NewStrategyTriggerResource() resource.Resource {
	return &strategyTriggerResource{}
}

func (r *strategyTriggerResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_strategy_trigger"
}

func (r *strategyTriggerResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		MarkdownDescription: "t-rader の戦略 trigger を管理します。",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Computed:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.UseStateForUnknown()},
				MarkdownDescription: "API が発行する trigger UUID。",
			},
			"strategy_id": schema.StringAttribute{
				Required:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.RequiresReplace()},
				MarkdownDescription: "親となる戦略 UUID。変更時は trigger を再作成します。",
			},
			"purpose": schema.StringAttribute{
				Optional:            true,
				MarkdownDescription: "タスクの投入先となる agent 設定。省略時は default を使用します。",
			},
			"kind": schema.StringAttribute{
				Required:            true,
				Validators:          []validator.String{triggerKindValidator{}},
				PlanModifiers:       []planmodifier.String{stringplanmodifier.RequiresReplace()},
				MarkdownDescription: "trigger の種別 (`cron` または `hook`)。変更時は trigger を再作成します。",
			},
			"schedule": schema.StringAttribute{
				Optional:            true,
				MarkdownDescription: "`kind = \"cron\"` の場合に必要な UTC の 5 フィールド cron 式。",
			},
			"hook_slug": schema.StringAttribute{
				Optional:            true,
				MarkdownDescription: "`kind = \"hook\"` の場合に必要な hook slug。",
			},
			"event_match": schema.DynamicAttribute{
				Optional:            true,
				MarkdownDescription: "hook payload の照合条件。更新時に設定から外すと条件を解除します。",
			},
			"prompt_template": schema.StringAttribute{
				Required:            true,
				MarkdownDescription: "実行時に使用する prompt template。",
			},
			"enabled": schema.BoolAttribute{
				Optional:            true,
				Computed:            true,
				MarkdownDescription: "trigger の有効状態。新規作成時に省略すると API の既定値を使用します。",
			},
			"created_at": schema.StringAttribute{
				Computed:            true,
				MarkdownDescription: "作成日時 (RFC 3339)。",
			},
			"updated_at": schema.StringAttribute{
				Computed:            true,
				MarkdownDescription: "更新日時 (RFC 3339)。",
			},
		},
	}
}

func (r *strategyTriggerResource) ConfigValidators(_ context.Context) []resource.ConfigValidator {
	return []resource.ConfigValidator{strategyTriggerConfigValidator{}}
}

func (r *strategyTriggerResource) Configure(_ context.Context, req resource.ConfigureRequest, resp *resource.ConfigureResponse) {
	if req.ProviderData == nil {
		return
	}
	client, ok := req.ProviderData.(*traderapi.Client)
	if !ok {
		resp.Diagnostics.AddError("Unexpected Resource Configure Type", fmt.Sprintf("Expected *traderapi.Client, got %T.", req.ProviderData))
		return
	}
	r.client = client
}

func (r *strategyTriggerResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan strategyTriggerModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}
	eventMatch, err := eventMatchForCreate(ctx, plan.EventMatch)
	if err != nil {
		resp.Diagnostics.AddAttributeError(path.Root("event_match"), "Invalid event_match", err.Error())
		return
	}
	created, err := client.CreateStrategyTrigger(ctx, plan.StrategyID.ValueString(), traderapigen.CreateTriggerRequest{
		Enabled:        boolAttributeNullable(plan.Enabled),
		EventMatch:     eventMatch,
		HookSlug:       stringAttributeNullable(plan.HookSlug),
		Kind:           traderapigen.TriggerKind(plan.Kind.ValueString()),
		Purpose:        stringAttributeNullable(plan.Purpose),
		PromptTemplate: plan.PromptTemplate.ValueString(),
		Schedule:       stringAttributeNullable(plan.Schedule),
	})
	if err != nil {
		resp.Diagnostics.AddError("Error creating strategy trigger", err.Error())
		return
	}
	model, err := modelFromStrategyTrigger(ctx, created)
	if err != nil {
		resp.Diagnostics.AddError("Error reading created strategy trigger", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, model)...)
}

func (r *strategyTriggerResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state strategyTriggerModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	trigger, err := client.GetTrigger(ctx, state.ID.ValueString())
	if errors.Is(err, traderapi.ErrNotFound) {
		resp.State.RemoveResource(ctx)
		return
	}
	if err != nil {
		resp.Diagnostics.AddError("Error reading strategy trigger", err.Error())
		return
	}
	model, err := modelFromStrategyTrigger(ctx, trigger)
	if err != nil {
		resp.Diagnostics.AddError("Error reading strategy trigger", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, model)...)
}

func (r *strategyTriggerResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan strategyTriggerModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	var state strategyTriggerModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}
	eventMatch, err := eventMatchForUpdate(ctx, plan.EventMatch)
	if err != nil {
		resp.Diagnostics.AddAttributeError(path.Root("event_match"), "Invalid event_match", err.Error())
		return
	}
	updated, err := client.UpdateTrigger(ctx, state.ID.ValueString(), traderapigen.UpdateTriggerRequest{
		Enabled:        boolAttributeNullable(plan.Enabled),
		EventMatch:     eventMatch,
		HookSlug:       stringAttributeUpdateNullable(plan.HookSlug),
		Purpose:        stringAttributeUpdateNullable(plan.Purpose),
		PromptTemplate: stringAttributeUpdateNullable(plan.PromptTemplate),
		Schedule:       stringAttributeUpdateNullable(plan.Schedule),
	})
	if err != nil {
		resp.Diagnostics.AddError("Error updating strategy trigger", err.Error())
		return
	}
	model, err := modelFromStrategyTrigger(ctx, updated)
	if err != nil {
		resp.Diagnostics.AddError("Error reading updated strategy trigger", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, model)...)
}

func (r *strategyTriggerResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state strategyTriggerModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}
	if err := client.DeleteTrigger(ctx, state.ID.ValueString()); err != nil && !errors.Is(err, traderapi.ErrNotFound) {
		resp.Diagnostics.AddError("Error deleting strategy trigger", err.Error())
	}
}

func (r *strategyTriggerResource) ImportState(ctx context.Context, req resource.ImportStateRequest, resp *resource.ImportStateResponse) {
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("id"), req.ID)...)
}

func (r *strategyTriggerResource) configuredClient(diagnostics *diag.Diagnostics) (*traderapi.Client, bool) {
	if r.client == nil {
		diagnostics.AddError("Provider Not Configured", "The t-rader provider must be configured before managing strategy triggers.")
		return nil, false
	}
	return r.client, true
}

func modelFromStrategyTrigger(ctx context.Context, trigger traderapigen.Trigger) (strategyTriggerModel, error) {
	eventMatch, err := dynamicEventMatchFromAPI(ctx, trigger.EventMatch)
	if err != nil {
		return strategyTriggerModel{}, err
	}
	strategyID := uuidNullableAttribute(trigger.StrategyId)
	if strategyID.IsNull() {
		return strategyTriggerModel{}, errors.New("backend returned a trigger without strategy_id")
	}
	return strategyTriggerModel{
		ID:             types.StringValue(trigger.TriggerId.String()),
		StrategyID:     strategyID,
		Purpose:        stringNullableAttribute(trigger.Purpose),
		Kind:           types.StringValue(trigger.Kind),
		Schedule:       stringNullableAttribute(trigger.Schedule),
		HookSlug:       stringNullableAttribute(trigger.HookSlug),
		EventMatch:     eventMatch,
		PromptTemplate: types.StringValue(trigger.PromptTemplate),
		Enabled:        types.BoolValue(trigger.Enabled),
		CreatedAt:      types.StringValue(trigger.CreatedAt.Format(time.RFC3339Nano)),
		UpdatedAt:      types.StringValue(trigger.UpdatedAt.Format(time.RFC3339Nano)),
	}, nil
}

func boolAttributeNullable(value types.Bool) nullable.Nullable[bool] {
	if value.IsNull() || value.IsUnknown() {
		return nullable.Nullable[bool]{}
	}
	return nullable.NewNullableWithValue(value.ValueBool())
}

func uuidNullableAttribute(value nullable.Nullable[openapi_types.UUID]) types.String {
	if !value.IsSpecified() || value.IsNull() {
		return types.StringNull()
	}
	return types.StringValue(value.GetOrEmpty().String())
}

type triggerKindValidator struct{}

func (triggerKindValidator) Description(context.Context) string {
	return "kind は cron または hook にしてください。"
}

func (triggerKindValidator) MarkdownDescription(ctx context.Context) string {
	return triggerKindValidator{}.Description(ctx)
}

func (triggerKindValidator) ValidateString(_ context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	switch req.ConfigValue.ValueString() {
	case "cron", "hook":
		return
	default:
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid trigger kind", "kind は cron または hook にしてください。")
	}
}

type strategyTriggerConfigValidator struct{}

func (strategyTriggerConfigValidator) Description(context.Context) string {
	return "kind に応じて schedule または hook_slug のいずれかを指定してください。"
}

func (strategyTriggerConfigValidator) MarkdownDescription(ctx context.Context) string {
	return strategyTriggerConfigValidator{}.Description(ctx)
}

func (strategyTriggerConfigValidator) ValidateResource(ctx context.Context, req resource.ValidateConfigRequest, resp *resource.ValidateConfigResponse) {
	var config strategyTriggerModel
	resp.Diagnostics.Append(req.Config.Get(ctx, &config)...)
	if resp.Diagnostics.HasError() {
		return
	}
	if config.Kind.IsNull() || config.Kind.IsUnknown() {
		return
	}
	if !config.EventMatch.IsNull() && !config.EventMatch.IsUnknown() {
		switch config.EventMatch.UnderlyingValue().(type) {
		case types.Object, types.Map:
		default:
			resp.Diagnostics.AddAttributeError(path.Root("event_match"), "Invalid event_match", "event_match には object を指定してください。")
		}
	}

	scheduleUnknown := config.Schedule.IsUnknown()
	hookSlugUnknown := config.HookSlug.IsUnknown()
	if scheduleUnknown || hookSlugUnknown {
		return
	}
	switch config.Kind.ValueString() {
	case "cron":
		if config.Schedule.IsNull() {
			resp.Diagnostics.AddAttributeError(path.Root("schedule"), "Missing schedule", "kind が cron の場合は schedule を指定してください。")
		}
		if !config.HookSlug.IsNull() {
			resp.Diagnostics.AddAttributeError(path.Root("hook_slug"), "Unexpected hook_slug", "kind が cron の場合は hook_slug を省略してください。")
		}
	case "hook":
		if config.HookSlug.IsNull() {
			resp.Diagnostics.AddAttributeError(path.Root("hook_slug"), "Missing hook_slug", "kind が hook の場合は hook_slug を指定してください。")
		}
		if !config.Schedule.IsNull() {
			resp.Diagnostics.AddAttributeError(path.Root("schedule"), "Unexpected schedule", "kind が hook の場合は schedule を省略してください。")
		}
	}
}
