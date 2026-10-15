package provider

import (
	"context"
	"errors"
	"fmt"
	"regexp"

	"github.com/hashicorp/terraform-plugin-framework/diag"
	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/stringplanmodifier"
	"github.com/hashicorp/terraform-plugin-framework/schema/validator"
	"github.com/hashicorp/terraform-plugin-framework/types"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
	traderapigen "github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

var (
	_ resource.Resource                   = (*subagentDefinitionResource)(nil)
	_ resource.ResourceWithImportState    = (*subagentDefinitionResource)(nil)
	_ resource.ResourceWithValidateConfig = (*subagentDefinitionResource)(nil)

	subagentDefinitionKeyPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9_-]*$`)
)

type subagentDefinitionResource struct {
	client *traderapi.Client
}

type subagentDefinitionModel struct {
	Key             types.String  `tfsdk:"key"`
	Description     types.String  `tfsdk:"description"`
	Model           types.String  `tfsdk:"model"`
	ReasoningEffort types.String  `tfsdk:"reasoning_effort"`
	Prompt          types.String  `tfsdk:"prompt"`
	Tools           types.List    `tfsdk:"tools"`
	TimeoutMinutes  types.Int32   `tfsdk:"timeout_minutes"`
	MaxModelCalls   types.Int64   `tfsdk:"max_model_calls"`
	ToolLimits      types.Map     `tfsdk:"tool_limits"`
	Output          types.Dynamic `tfsdk:"output"`
}

type subagentDefinitionFields struct {
	description    string
	model          string
	prompt         string
	tools          []string
	timeoutMinutes int32
	maxModelCalls  int64
	toolLimits     map[string]int64
	output         map[string]interface{}
}

func NewSubagentDefinitionResource() resource.Resource {
	return &subagentDefinitionResource{}
}

func (r *subagentDefinitionResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_subagent"
}

func (r *subagentDefinitionResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		MarkdownDescription: "サブエージェント定義を管理します。",
		Attributes: map[string]schema.Attribute{
			"key": schema.StringAttribute{
				Required:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.RequiresReplace()},
				Validators:          []validator.String{subagentDefinitionKeyValidator{}},
				MarkdownDescription: "定義を識別する key。変更するとリソースを再作成します。import 時は key を指定します。",
			},
			"description": schema.StringAttribute{
				Required:            true,
				MarkdownDescription: "定義の説明。",
			},
			"model": schema.StringAttribute{
				Required:            true,
				MarkdownDescription: "サブエージェントで使用するモデル名。",
			},
			"reasoning_effort": schema.StringAttribute{
				Optional:            true,
				MarkdownDescription: "モデルに渡す推論 effort。",
			},
			"prompt": schema.StringAttribute{
				Required:            true,
				MarkdownDescription: "サブエージェントに渡す指示文。",
			},
			"tools": schema.ListAttribute{
				Required:            true,
				ElementType:         types.StringType,
				MarkdownDescription: "使用できる tool 名の一覧。",
			},
			"timeout_minutes": schema.Int32Attribute{
				Required:            true,
				Validators:          []validator.Int32{subagentTimeoutMinutesValidator{}},
				MarkdownDescription: "実行 timeout (1 から 360)。",
			},
			"max_model_calls": schema.Int64Attribute{
				Required:            true,
				Validators:          []validator.Int64{subagentMaxModelCallsValidator{}},
				MarkdownDescription: "許可する最大 model 呼び出し回数。",
			},
			"tool_limits": schema.MapAttribute{
				Required:            true,
				ElementType:         types.Int64Type,
				MarkdownDescription: "tool 名ごとの呼び出し上限。",
			},
			"output": schema.DynamicAttribute{
				Required:            true,
				MarkdownDescription: "サブエージェントの出力形式を表す JSON object。",
			},
		},
	}
}

func (r *subagentDefinitionResource) Configure(_ context.Context, req resource.ConfigureRequest, resp *resource.ConfigureResponse) {
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

func (r *subagentDefinitionResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan subagentDefinitionModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}
	fields := subagentDefinitionFieldsFromPlan(ctx, plan, &resp.Diagnostics)
	if resp.Diagnostics.HasError() {
		return
	}

	created, err := client.CreateSubagentDefinition(ctx, traderapigen.CreateSubagentDefinitionRequest{
		Description:     fields.description,
		Key:             plan.Key.ValueString(),
		MaxModelCalls:   fields.maxModelCalls,
		Model:           fields.model,
		Output:          fields.output,
		Prompt:          fields.prompt,
		ReasoningEffort: stringAttributeNullable(plan.ReasoningEffort),
		TimeoutMinutes:  fields.timeoutMinutes,
		ToolLimits:      fields.toolLimits,
		Tools:           fields.tools,
	})
	if err != nil {
		resp.Diagnostics.AddError("Error creating subagent definition", err.Error())
		return
	}
	model, diagnostics := modelFromSubagentDefinition(ctx, created, &plan.Output)
	resp.Diagnostics.Append(diagnostics...)
	if resp.Diagnostics.HasError() {
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, model)...)
}

func (r *subagentDefinitionResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state subagentDefinitionModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	definition, err := client.GetSubagentDefinition(ctx, state.Key.ValueString())
	if errors.Is(err, traderapi.ErrNotFound) {
		resp.State.RemoveResource(ctx)
		return
	}
	if err != nil {
		resp.Diagnostics.AddError("Error reading subagent definition", err.Error())
		return
	}
	model, diagnostics := modelFromSubagentDefinition(ctx, definition, &state.Output)
	resp.Diagnostics.Append(diagnostics...)
	if resp.Diagnostics.HasError() {
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, model)...)
}

func (r *subagentDefinitionResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan subagentDefinitionModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	var state subagentDefinitionModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}
	fields := subagentDefinitionFieldsFromPlan(ctx, plan, &resp.Diagnostics)
	if resp.Diagnostics.HasError() {
		return
	}
	updated, err := client.UpdateSubagentDefinition(ctx, state.Key.ValueString(), traderapigen.UpdateSubagentDefinitionRequest{
		Description:     fields.description,
		MaxModelCalls:   fields.maxModelCalls,
		Model:           fields.model,
		Output:          fields.output,
		Prompt:          fields.prompt,
		ReasoningEffort: stringAttributeUpdateNullable(plan.ReasoningEffort),
		TimeoutMinutes:  fields.timeoutMinutes,
		ToolLimits:      fields.toolLimits,
		Tools:           fields.tools,
	})
	if err != nil {
		resp.Diagnostics.AddError("Error updating subagent definition", err.Error())
		return
	}
	model, diagnostics := modelFromSubagentDefinition(ctx, updated, &plan.Output)
	resp.Diagnostics.Append(diagnostics...)
	if resp.Diagnostics.HasError() {
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, model)...)
}

func (r *subagentDefinitionResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state subagentDefinitionModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}
	if err := client.DeleteSubagentDefinition(ctx, state.Key.ValueString()); err != nil && !errors.Is(err, traderapi.ErrNotFound) {
		resp.Diagnostics.AddError("Error deleting subagent definition", err.Error())
	}
}

func (r *subagentDefinitionResource) ImportState(ctx context.Context, req resource.ImportStateRequest, resp *resource.ImportStateResponse) {
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("key"), req.ID)...)
}

func (r *subagentDefinitionResource) ValidateConfig(ctx context.Context, req resource.ValidateConfigRequest, resp *resource.ValidateConfigResponse) {
	var config subagentDefinitionModel
	resp.Diagnostics.Append(req.Config.Get(ctx, &config)...)
	if resp.Diagnostics.HasError() || config.Output.IsNull() {
		return
	}
	underlying := config.Output.UnderlyingValue()
	if underlying == nil {
		return
	}
	terraformValue, err := underlying.ToTerraformValue(ctx)
	if err != nil {
		resp.Diagnostics.AddAttributeError(path.Root("output"), "Invalid output", fmt.Sprintf("convert Terraform value: %s", err))
		return
	}
	if !terraformValue.IsFullyKnown() {
		return
	}
	if _, err := jsonObjectFromDynamic(ctx, config.Output); err != nil {
		resp.Diagnostics.AddAttributeError(path.Root("output"), "Invalid output", err.Error())
	}
}

func (r *subagentDefinitionResource) configuredClient(diagnostics *diag.Diagnostics) (*traderapi.Client, bool) {
	if r.client == nil {
		diagnostics.AddError("Provider Not Configured", "The t-rader provider must be configured before managing subagent definitions.")
		return nil, false
	}
	return r.client, true
}

func subagentDefinitionFieldsFromPlan(ctx context.Context, plan subagentDefinitionModel, diagnostics *diag.Diagnostics) subagentDefinitionFields {
	var fields subagentDefinitionFields
	fields.description = plan.Description.ValueString()
	fields.model = plan.Model.ValueString()
	fields.prompt = plan.Prompt.ValueString()
	fields.timeoutMinutes = plan.TimeoutMinutes.ValueInt32()
	fields.maxModelCalls = plan.MaxModelCalls.ValueInt64()

	diagnostics.Append(plan.Tools.ElementsAs(ctx, &fields.tools, false)...)
	fields.tools = nonNilSlice(fields.tools)
	diagnostics.Append(plan.ToolLimits.ElementsAs(ctx, &fields.toolLimits, false)...)
	fields.toolLimits = nonNilMap(fields.toolLimits)

	output, err := jsonObjectFromDynamic(ctx, plan.Output)
	if err != nil {
		diagnostics.AddAttributeError(path.Root("output"), "Invalid output", err.Error())
		return fields
	}
	fields.output = output
	return fields
}

func modelFromSubagentDefinition(ctx context.Context, definition traderapigen.SubagentDefinitionResponse, priorOutput *types.Dynamic) (subagentDefinitionModel, diag.Diagnostics) {
	var diagnostics diag.Diagnostics
	output, err := dynamicFromJSON(definition.Output)
	if err != nil {
		diagnostics.AddError("Error decoding subagent definition output", err.Error())
		return subagentDefinitionModel{}, diagnostics
	}
	if priorOutput != nil && jsonObjectMatchesPrior(ctx, *priorOutput, definition.Output) {
		output = *priorOutput
	}
	tools, toolDiagnostics := types.ListValueFrom(ctx, types.StringType, nonNilSlice(definition.Tools))
	diagnostics.Append(toolDiagnostics...)
	toolLimits, limitDiagnostics := types.MapValueFrom(ctx, types.Int64Type, nonNilMap(definition.ToolLimits))
	diagnostics.Append(limitDiagnostics...)

	return subagentDefinitionModel{
		Key:             types.StringValue(definition.Key),
		Description:     types.StringValue(definition.Description),
		Model:           types.StringValue(definition.Model),
		ReasoningEffort: stringNullableAttribute(definition.ReasoningEffort),
		Prompt:          types.StringValue(definition.Prompt),
		Tools:           tools,
		TimeoutMinutes:  types.Int32Value(definition.TimeoutMinutes),
		MaxModelCalls:   types.Int64Value(definition.MaxModelCalls),
		ToolLimits:      toolLimits,
		Output:          output,
	}, diagnostics
}

func nonNilSlice[T any](values []T) []T {
	if values == nil {
		return []T{}
	}
	return values
}

func nonNilMap[K comparable, V any](values map[K]V) map[K]V {
	if values == nil {
		return map[K]V{}
	}
	return values
}

type subagentDefinitionKeyValidator struct{}

func (subagentDefinitionKeyValidator) Description(context.Context) string {
	return "key は小文字英数字、ハイフン、アンダースコアで指定し、小文字英数字から始めてください。"
}

func (v subagentDefinitionKeyValidator) MarkdownDescription(ctx context.Context) string {
	return v.Description(ctx)
}

func (subagentDefinitionKeyValidator) ValidateString(_ context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	if !subagentDefinitionKeyPattern.MatchString(req.ConfigValue.ValueString()) {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid subagent definition key", "Key must match ^[a-z0-9][a-z0-9_-]*$.")
	}
}

type subagentTimeoutMinutesValidator struct{}

func (subagentTimeoutMinutesValidator) Description(context.Context) string {
	return "timeout_minutes は 1 から 360 の範囲で指定してください。"
}

func (v subagentTimeoutMinutesValidator) MarkdownDescription(ctx context.Context) string {
	return v.Description(ctx)
}

func (subagentTimeoutMinutesValidator) ValidateInt32(_ context.Context, req validator.Int32Request, resp *validator.Int32Response) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	value := req.ConfigValue.ValueInt32()
	if value < 1 || value > 360 {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid timeout_minutes", "timeout_minutes must be between 1 and 360.")
	}
}

type subagentMaxModelCallsValidator struct{}

func (subagentMaxModelCallsValidator) Description(context.Context) string {
	return "max_model_calls は 1 以上で指定してください。"
}

func (v subagentMaxModelCallsValidator) MarkdownDescription(ctx context.Context) string {
	return v.Description(ctx)
}

func (subagentMaxModelCallsValidator) ValidateInt64(_ context.Context, req validator.Int64Request, resp *validator.Int64Response) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	if req.ConfigValue.ValueInt64() < 1 {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid max_model_calls", "max_model_calls must be at least 1.")
	}
}
