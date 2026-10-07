package provider

import (
	"context"
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/google/uuid"
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
	_ resource.Resource                = (*strategyEarningsTargetResource)(nil)
	_ resource.ResourceWithImportState = (*strategyEarningsTargetResource)(nil)
)

type strategyEarningsTargetResource struct {
	client *traderapi.Client
}

type strategyEarningsTargetModel struct {
	ID         types.String `tfsdk:"id"`
	StrategyID types.String `tfsdk:"strategy_id"`
	RefKind    types.String `tfsdk:"ref_kind"`
	RefID      types.String `tfsdk:"ref_id"`
	CreatedAt  types.String `tfsdk:"created_at"`
}

func NewStrategyEarningsTargetResource() resource.Resource {
	return &strategyEarningsTargetResource{}
}

func (r *strategyEarningsTargetResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_strategy_earnings_target"
}

func (r *strategyEarningsTargetResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		MarkdownDescription: "戦略が決算を追う銘柄またはグループを管理します。",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Computed:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.UseStateForUnknown()},
				MarkdownDescription: "`strategy_id/ref_kind/ref_id` 形式の識別子。import 時もこの形式を指定します。",
			},
			"strategy_id": schema.StringAttribute{
				Required:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.RequiresReplace()},
				MarkdownDescription: "親となる戦略 UUID。変更時は登録を作り直します。",
			},
			"ref_kind": schema.StringAttribute{
				Required:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.RequiresReplace()},
				Validators:          []validator.String{strategyEarningsTargetRefKindValidator{}},
				MarkdownDescription: "対象の種類 (`stock` または `group`)。変更時は登録を作り直します。",
			},
			"ref_id": schema.StringAttribute{
				Required:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.RequiresReplace()},
				Validators:          []validator.String{strategyEarningsTargetRefIDValidator{}},
				MarkdownDescription: "銘柄 ID または `軸の key/グループの key`。変更時は登録を作り直します。",
			},
			"created_at": schema.StringAttribute{
				Computed:            true,
				MarkdownDescription: "登録日時 (RFC 3339)。",
			},
		},
	}
}

func (r *strategyEarningsTargetResource) Configure(_ context.Context, req resource.ConfigureRequest, resp *resource.ConfigureResponse) {
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

func (r *strategyEarningsTargetResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan strategyEarningsTargetModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	if err := client.AddStrategyEarningsTarget(ctx, plan.StrategyID.ValueString(), plan.RefKind.ValueString(), plan.RefID.ValueString()); err != nil {
		resp.Diagnostics.AddError("Error creating strategy earnings target", err.Error())
		return
	}
	target, found, err := findStrategyEarningsTarget(ctx, client, plan.StrategyID.ValueString(), plan.RefKind.ValueString(), plan.RefID.ValueString())
	if err != nil {
		resp.Diagnostics.AddError("Error reading created strategy earnings target", err.Error())
		return
	}
	if !found {
		resp.Diagnostics.AddError("Error reading created strategy earnings target", "The target was not present in the strategy earnings target list after the add request.")
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromStrategyEarningsTarget(plan.StrategyID.ValueString(), target))...)
}

func (r *strategyEarningsTargetResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state strategyEarningsTargetModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}
	target, found, err := findStrategyEarningsTarget(ctx, client, state.StrategyID.ValueString(), state.RefKind.ValueString(), state.RefID.ValueString())
	if errors.Is(err, traderapi.ErrNotFound) {
		resp.State.RemoveResource(ctx)
		return
	}
	if err != nil {
		resp.Diagnostics.AddError("Error reading strategy earnings target", err.Error())
		return
	}
	if !found {
		resp.State.RemoveResource(ctx)
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromStrategyEarningsTarget(state.StrategyID.ValueString(), target))...)
}

func (r *strategyEarningsTargetResource) Update(_ context.Context, _ resource.UpdateRequest, resp *resource.UpdateResponse) {
	resp.Diagnostics.AddError("Unexpected in-place update", "Changing strategy_id, ref_kind, or ref_id requires resource replacement.")
}

func (r *strategyEarningsTargetResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state strategyEarningsTargetModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}
	if err := client.RemoveStrategyEarningsTarget(ctx, state.StrategyID.ValueString(), state.RefKind.ValueString(), state.RefID.ValueString()); err != nil && !errors.Is(err, traderapi.ErrNotFound) {
		resp.Diagnostics.AddError("Error deleting strategy earnings target", err.Error())
	}
}

func (r *strategyEarningsTargetResource) ImportState(ctx context.Context, req resource.ImportStateRequest, resp *resource.ImportStateResponse) {
	parts := strings.SplitN(req.ID, "/", 3)
	if len(parts) != 3 || parts[0] == "" || parts[1] == "" || parts[2] == "" {
		resp.Diagnostics.AddError("Invalid Import ID", "Import ID must have the form <strategy_id>/<ref_kind>/<ref_id>.")
		return
	}
	if _, err := uuid.Parse(parts[0]); err != nil {
		resp.Diagnostics.AddError("Invalid Import ID", "The first import ID component must be a strategy UUID.")
		return
	}
	if parts[1] != "stock" && parts[1] != "group" {
		resp.Diagnostics.AddError("Invalid Import ID", "ref_kind in the import ID must be stock or group.")
		return
	}
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("id"), req.ID)...)
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("strategy_id"), parts[0])...)
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("ref_kind"), parts[1])...)
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("ref_id"), parts[2])...)
}

func (r *strategyEarningsTargetResource) configuredClient(diagnostics *diag.Diagnostics) (*traderapi.Client, bool) {
	if r.client == nil {
		diagnostics.AddError("Provider Not Configured", "The t-rader provider must be configured before managing strategy earnings targets.")
		return nil, false
	}
	return r.client, true
}

func findStrategyEarningsTarget(ctx context.Context, client *traderapi.Client, strategyID, refKind, refID string) (traderapigen.StrategyEarningsTargetResponse, bool, error) {
	targets, err := client.ListStrategyEarningsTargets(ctx, strategyID)
	if err != nil {
		return traderapigen.StrategyEarningsTargetResponse{}, false, err
	}
	for _, target := range targets {
		if target.RefKind == refKind && target.RefId == refID {
			return target, true, nil
		}
	}
	return traderapigen.StrategyEarningsTargetResponse{}, false, nil
}

func modelFromStrategyEarningsTarget(strategyID string, target traderapigen.StrategyEarningsTargetResponse) strategyEarningsTargetModel {
	return strategyEarningsTargetModel{
		ID:         types.StringValue(strategyEarningsTargetID(strategyID, target.RefKind, target.RefId)),
		StrategyID: types.StringValue(strategyID),
		RefKind:    types.StringValue(target.RefKind),
		RefID:      types.StringValue(target.RefId),
		CreatedAt:  types.StringValue(target.CreatedAt.Format(time.RFC3339Nano)),
	}
}

func strategyEarningsTargetID(strategyID, refKind, refID string) string {
	return strategyID + "/" + refKind + "/" + refID
}

type strategyEarningsTargetRefKindValidator struct{}

func (strategyEarningsTargetRefKindValidator) Description(context.Context) string {
	return "ref_kind は stock または group にしてください。"
}

func (v strategyEarningsTargetRefKindValidator) MarkdownDescription(ctx context.Context) string {
	return v.Description(ctx)
}

func (strategyEarningsTargetRefKindValidator) ValidateString(_ context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	switch req.ConfigValue.ValueString() {
	case "stock", "group":
		return
	default:
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid earnings target ref_kind", "ref_kind は stock または group にしてください。")
	}
}

type strategyEarningsTargetRefIDValidator struct{}

func (strategyEarningsTargetRefIDValidator) Description(context.Context) string {
	return "ref_id は空にできません。"
}

func (v strategyEarningsTargetRefIDValidator) MarkdownDescription(ctx context.Context) string {
	return v.Description(ctx)
}

func (strategyEarningsTargetRefIDValidator) ValidateString(_ context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() || req.ConfigValue.ValueString() != "" {
		return
	}
	resp.Diagnostics.AddAttributeError(req.Path, "Invalid earnings target ref_id", "ref_id は空にできません。")
}
