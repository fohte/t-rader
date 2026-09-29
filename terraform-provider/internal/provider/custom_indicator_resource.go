package provider

import (
	"context"
	"errors"
	"fmt"
	"strings"
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

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
)

var (
	_ resource.Resource                = (*customIndicatorResource)(nil)
	_ resource.ResourceWithImportState = (*customIndicatorResource)(nil)
)

type customIndicatorResource struct {
	client *traderapi.Client
}

type customIndicatorModel struct {
	ID           types.String  `tfsdk:"id"`
	Name         types.String  `tfsdk:"name"`
	Code         types.String  `tfsdk:"code"`
	InputSchema  types.Dynamic `tfsdk:"input_schema"`
	OutputSchema types.Dynamic `tfsdk:"output_schema"`
	Description  types.String  `tfsdk:"description"`
	StrategyID   types.String  `tfsdk:"strategy_id"`
	Scope        types.String  `tfsdk:"scope"`
	CreatedAt    types.String  `tfsdk:"created_at"`
	UpdatedAt    types.String  `tfsdk:"updated_at"`
}

func NewCustomIndicatorResource() resource.Resource {
	return &customIndicatorResource{}
}

func (r *customIndicatorResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_custom_indicator"
}

func (r *customIndicatorResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		MarkdownDescription: "カスタム指標を管理します。`strategy_id` を省略するとグローバル指標になります。",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Computed:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.UseStateForUnknown()},
				MarkdownDescription: "バックエンドが発行する指標 UUID。",
			},
			"name": schema.StringAttribute{
				Required:            true,
				Validators:          []validator.String{customIndicatorNameValidator{}},
				MarkdownDescription: "指標名。前後の空白は指定できません。",
			},
			"code": schema.StringAttribute{
				Required:            true,
				MarkdownDescription: "指標の実行コード。",
			},
			"input_schema": schema.DynamicAttribute{
				Required:            true,
				MarkdownDescription: "指標への入力を表す JSON object。",
			},
			"output_schema": schema.DynamicAttribute{
				Required:            true,
				MarkdownDescription: "指標の出力を表す JSON object。",
			},
			"description": schema.StringAttribute{
				Optional:            true,
				MarkdownDescription: "説明。更新時に null を指定すると削除します。",
			},
			"strategy_id": schema.StringAttribute{
				Optional:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.RequiresReplace()},
				MarkdownDescription: "戦略専用指標にする場合の戦略 UUID。変更時は指標を作り直します。",
			},
			"scope": schema.StringAttribute{
				Computed:            true,
				MarkdownDescription: "指標の scope (`global` または `strategy`)。",
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

func (r *customIndicatorResource) Configure(_ context.Context, req resource.ConfigureRequest, resp *resource.ConfigureResponse) {
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

func (r *customIndicatorResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan customIndicatorModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	inputSchema, outputSchema, schemaDiagnostics := customIndicatorPlanSchemas(ctx, plan)
	resp.Diagnostics.Append(schemaDiagnostics...)
	if resp.Diagnostics.HasError() {
		return
	}

	strategyID, err := customIndicatorStrategyID(plan.StrategyID)
	if err != nil {
		resp.Diagnostics.AddAttributeError(path.Root("strategy_id"), "Invalid strategy ID", err.Error())
		return
	}

	created, err := client.CreateCustomIndicator(ctx, strategyID, traderapi.CreateCustomIndicatorRequest{
		Name:         plan.Name.ValueString(),
		Code:         plan.Code.ValueString(),
		InputSchema:  inputSchema,
		OutputSchema: outputSchema,
		Description:  stringAttributeNullable(plan.Description),
	})
	if err != nil {
		resp.Diagnostics.AddError("Error creating custom indicator", err.Error())
		return
	}
	model, err := modelFromCustomIndicatorWithPlan(created, plan)
	if err != nil {
		resp.Diagnostics.AddError("Error reading created custom indicator", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, model)...)
}

func (r *customIndicatorResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state customIndicatorModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	indicator, err := client.GetCustomIndicator(ctx, state.ID.ValueString())
	if errors.Is(err, traderapi.ErrNotFound) {
		resp.State.RemoveResource(ctx)
		return
	}
	if err != nil {
		resp.Diagnostics.AddError("Error reading custom indicator", err.Error())
		return
	}
	model, err := modelFromCustomIndicator(indicator)
	if err != nil {
		resp.Diagnostics.AddError("Error reading custom indicator", err.Error())
		return
	}
	if customIndicatorSchemaMatches(ctx, state.InputSchema, indicator.InputSchema) {
		model.InputSchema = state.InputSchema
	}
	if customIndicatorSchemaMatches(ctx, state.OutputSchema, indicator.OutputSchema) {
		model.OutputSchema = state.OutputSchema
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, model)...)
}

func (r *customIndicatorResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan customIndicatorModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	var state customIndicatorModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	inputSchema, outputSchema, schemaDiagnostics := customIndicatorPlanSchemas(ctx, plan)
	resp.Diagnostics.Append(schemaDiagnostics...)
	if resp.Diagnostics.HasError() {
		return
	}

	updated, err := client.UpdateCustomIndicator(ctx, state.ID.ValueString(), traderapi.UpdateCustomIndicatorRequest{
		Name:         stringAttributeUpdateNullable(plan.Name),
		Code:         stringAttributeUpdateNullable(plan.Code),
		InputSchema:  nullable.NewNullableWithValue(inputSchema),
		OutputSchema: nullable.NewNullableWithValue(outputSchema),
		Description:  stringAttributeUpdateNullable(plan.Description),
	})
	if err != nil {
		if errors.Is(err, traderapi.ErrNotFound) {
			resp.Diagnostics.AddError("Error updating custom indicator", "The custom indicator no longer exists.")
			return
		}
		resp.Diagnostics.AddError("Error updating custom indicator", err.Error())
		return
	}
	model, err := modelFromCustomIndicatorWithPlan(updated, plan)
	if err != nil {
		resp.Diagnostics.AddError("Error reading updated custom indicator", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, model)...)
}

func (r *customIndicatorResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state customIndicatorModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}
	if err := client.DeleteCustomIndicator(ctx, state.ID.ValueString()); err != nil && !errors.Is(err, traderapi.ErrNotFound) {
		resp.Diagnostics.AddError("Error deleting custom indicator", err.Error())
	}
}

func (r *customIndicatorResource) ImportState(ctx context.Context, req resource.ImportStateRequest, resp *resource.ImportStateResponse) {
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("id"), req.ID)...)
}

func (r *customIndicatorResource) configuredClient(diagnostics *diag.Diagnostics) (*traderapi.Client, bool) {
	if r.client == nil {
		diagnostics.AddError("Provider Not Configured", "The t-rader provider must be configured before managing custom indicators.")
		return nil, false
	}
	return r.client, true
}

func modelFromCustomIndicator(indicator traderapi.CustomIndicator) (customIndicatorModel, error) {
	inputSchema, err := customIndicatorDynamic(indicator.InputSchema)
	if err != nil {
		return customIndicatorModel{}, fmt.Errorf("decode input_schema: %w", err)
	}
	outputSchema, err := customIndicatorDynamic(indicator.OutputSchema)
	if err != nil {
		return customIndicatorModel{}, fmt.Errorf("decode output_schema: %w", err)
	}
	strategyID := types.StringNull()
	if indicator.StrategyId.IsSpecified() && !indicator.StrategyId.IsNull() {
		strategyID = types.StringValue(indicator.StrategyId.GetOrEmpty().String())
	}
	return customIndicatorModel{
		ID:           types.StringValue(indicator.IndicatorId.String()),
		Name:         types.StringValue(indicator.Name),
		Code:         types.StringValue(indicator.Code),
		InputSchema:  inputSchema,
		OutputSchema: outputSchema,
		Description:  stringNullableAttribute(indicator.Description),
		StrategyID:   strategyID,
		Scope:        types.StringValue(indicator.Scope),
		CreatedAt:    types.StringValue(indicator.CreatedAt.Format(time.RFC3339Nano)),
		UpdatedAt:    types.StringValue(indicator.UpdatedAt.Format(time.RFC3339Nano)),
	}, nil
}

func modelFromCustomIndicatorWithPlan(indicator traderapi.CustomIndicator, plan customIndicatorModel) (customIndicatorModel, error) {
	model, err := modelFromCustomIndicator(indicator)
	if err != nil {
		return customIndicatorModel{}, err
	}
	model.InputSchema = plan.InputSchema
	model.OutputSchema = plan.OutputSchema
	return model, nil
}

func customIndicatorStrategyID(value types.String) (*string, error) {
	if value.IsUnknown() {
		return nil, errors.New("strategy_id must be known when creating a custom indicator")
	}
	if value.IsNull() {
		return nil, nil
	}
	strategyID := value.ValueString()
	return &strategyID, nil
}

type customIndicatorNameValidator struct{}

func (customIndicatorNameValidator) Description(context.Context) string {
	return "指標名に前後の空白を含めないでください。"
}

func (v customIndicatorNameValidator) MarkdownDescription(ctx context.Context) string {
	return v.Description(ctx)
}

func (customIndicatorNameValidator) ValidateString(_ context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	value := req.ConfigValue.ValueString()
	if strings.TrimSpace(value) == "" {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid custom indicator name", "Custom indicator names cannot be empty or contain only whitespace.")
		return
	}
	if strings.TrimSpace(value) != value {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid custom indicator name", "Custom indicator names cannot start or end with whitespace because the API trims names.")
	}
}
