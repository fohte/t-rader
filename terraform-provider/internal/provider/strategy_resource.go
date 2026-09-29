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
	_ resource.Resource                = (*strategyResource)(nil)
	_ resource.ResourceWithImportState = (*strategyResource)(nil)
)

type strategyResource struct {
	client *traderapi.Client
}

type strategyModel struct {
	ID          types.String `tfsdk:"id"`
	Name        types.String `tfsdk:"name"`
	Description types.String `tfsdk:"description"`
	SortOrder   types.Int32  `tfsdk:"sort_order"`
	CreatedAt   types.String `tfsdk:"created_at"`
	UpdatedAt   types.String `tfsdk:"updated_at"`
}

func NewStrategyResource() resource.Resource {
	return &strategyResource{}
}

func (r *strategyResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_strategy"
}

func (r *strategyResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		MarkdownDescription: "t-rader の戦略を管理します。",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Computed:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.UseStateForUnknown()},
				MarkdownDescription: "バックエンドが発行する戦略 UUID。",
			},
			"name": schema.StringAttribute{
				Required:            true,
				Validators:          []validator.String{strategyNameValidator{}},
				MarkdownDescription: "戦略名。前後の空白は指定できません。",
			},
			"description": schema.StringAttribute{
				Optional:            true,
				MarkdownDescription: "戦略の説明。更新時に設定から外すと説明を削除します。",
			},
			"sort_order": schema.Int32Attribute{
				Optional:            true,
				Computed:            true,
				MarkdownDescription: "一覧表示順。省略時は API の現在値を保持し、新規作成時は API の既定値を使用します。",
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

func (r *strategyResource) Configure(_ context.Context, req resource.ConfigureRequest, resp *resource.ConfigureResponse) {
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

func (r *strategyResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan strategyModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	created, err := client.CreateStrategy(ctx, traderapi.CreateStrategyRequest{
		Name:        plan.Name.ValueString(),
		Description: stringAttributeNullable(plan.Description),
		SortOrder:   int32AttributeNullable(plan.SortOrder),
	})
	if err != nil {
		resp.Diagnostics.AddError("Error creating strategy", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromStrategy(created))...)
}

func (r *strategyResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state strategyModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	strategy, err := client.GetStrategy(ctx, state.ID.ValueString())
	if errors.Is(err, traderapi.ErrNotFound) {
		resp.State.RemoveResource(ctx)
		return
	}
	if err != nil {
		resp.Diagnostics.AddError("Error reading strategy", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromStrategy(strategy))...)
}

func (r *strategyResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan strategyModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	var state strategyModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	updated, err := client.UpdateStrategy(ctx, state.ID.ValueString(), traderapi.UpdateStrategyRequest{
		Name:        stringAttributeUpdateNullable(plan.Name),
		Description: stringAttributeUpdateNullable(plan.Description),
		SortOrder:   int32AttributeNullable(plan.SortOrder),
	})
	if err != nil {
		resp.Diagnostics.AddError("Error updating strategy", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromStrategy(updated))...)
}

func (r *strategyResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state strategyModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	if err := client.DeleteStrategy(ctx, state.ID.ValueString()); err != nil && !errors.Is(err, traderapi.ErrNotFound) {
		resp.Diagnostics.AddError("Error deleting strategy", err.Error())
	}
}

func (r *strategyResource) ImportState(ctx context.Context, req resource.ImportStateRequest, resp *resource.ImportStateResponse) {
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("id"), req.ID)...)
}

func (r *strategyResource) configuredClient(diagnostics *diag.Diagnostics) (*traderapi.Client, bool) {
	if r.client == nil {
		diagnostics.AddError("Provider Not Configured", "The t-rader provider must be configured before managing strategies.")
		return nil, false
	}
	return r.client, true
}

func modelFromStrategy(strategy traderapi.Strategy) strategyModel {
	return strategyModel{
		ID:          types.StringValue(strategy.Id.String()),
		Name:        types.StringValue(strategy.Name),
		Description: stringNullableAttribute(strategy.Description),
		SortOrder:   types.Int32Value(strategy.SortOrder),
		CreatedAt:   types.StringValue(strategy.CreatedAt.Format(time.RFC3339Nano)),
		UpdatedAt:   types.StringValue(strategy.UpdatedAt.Format(time.RFC3339Nano)),
	}
}

func stringAttributeNullable(value types.String) nullable.Nullable[string] {
	if value.IsNull() || value.IsUnknown() {
		return nullable.Nullable[string]{}
	}
	return nullable.NewNullableWithValue(value.ValueString())
}

func stringAttributeUpdateNullable(value types.String) nullable.Nullable[string] {
	if value.IsUnknown() {
		return nullable.Nullable[string]{}
	}
	if value.IsNull() {
		return nullable.NewNullNullable[string]()
	}
	return nullable.NewNullableWithValue(value.ValueString())
}

func int32AttributeNullable(value types.Int32) nullable.Nullable[int32] {
	if value.IsNull() || value.IsUnknown() {
		return nullable.Nullable[int32]{}
	}
	return nullable.NewNullableWithValue(value.ValueInt32())
}

func stringNullableAttribute(value nullable.Nullable[string]) types.String {
	if !value.IsSpecified() || value.IsNull() {
		return types.StringNull()
	}
	return types.StringValue(value.GetOrEmpty())
}

type strategyNameValidator struct{}

func (strategyNameValidator) Description(context.Context) string {
	return "戦略名に前後の空白を含めないでください。"
}

func (strategyNameValidator) MarkdownDescription(ctx context.Context) string {
	return strategyNameValidator{}.Description(ctx)
}

func (strategyNameValidator) ValidateString(_ context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	value := req.ConfigValue.ValueString()
	if strings.TrimSpace(value) == "" {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid strategy name", "Strategy names cannot be empty or contain only whitespace.")
		return
	}
	if strings.TrimSpace(value) != value {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid strategy name", "Strategy names cannot start or end with whitespace because the API trims names.")
	}
}
