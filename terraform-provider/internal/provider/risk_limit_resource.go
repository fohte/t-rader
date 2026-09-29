package provider

import (
	"context"
	"fmt"

	"github.com/hashicorp/terraform-plugin-framework/diag"
	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/stringplanmodifier"
	"github.com/hashicorp/terraform-plugin-framework/schema/validator"
	"github.com/hashicorp/terraform-plugin-framework/tfsdk"
	"github.com/hashicorp/terraform-plugin-framework/types"
	"github.com/oapi-codegen/nullable"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
)

const riskLimitResourceID = "account"

var (
	_ resource.Resource                = (*riskLimitResource)(nil)
	_ resource.ResourceWithImportState = (*riskLimitResource)(nil)
)

type riskLimitResource struct {
	client *traderapi.Client
}

type riskLimitModel struct {
	ID             types.String  `tfsdk:"id"`
	MaxSectorRatio types.Float64 `tfsdk:"max_sector_ratio"`
}

func NewRiskLimitResource() resource.Resource {
	return &riskLimitResource{}
}

func (r *riskLimitResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_risk_limit"
}

func (r *riskLimitResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		MarkdownDescription: "口座全体のリスク上限を管理します。",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Computed:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.UseStateForUnknown()},
				MarkdownDescription: "口座を識別する固定 ID (`account`)。",
			},
			"max_sector_ratio": schema.Float64Attribute{
				Optional:            true,
				Validators:          []validator.Float64{maxSectorRatioValidator{}},
				MarkdownDescription: "口座全体の保有銘柄時価に対する、単一セクターの保有銘柄時価の上限比率。`(0, 1]` の範囲で指定し、省略すると上限を解除します。",
			},
		},
	}
}

func (r *riskLimitResource) Configure(_ context.Context, req resource.ConfigureRequest, resp *resource.ConfigureResponse) {
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

func (r *riskLimitResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	r.upsert(ctx, req.Plan, &resp.State, &resp.Diagnostics, "Error creating risk limit")
}

func (r *riskLimitResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state riskLimitModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	current, err := client.GetRiskLimit(ctx)
	if err != nil {
		resp.Diagnostics.AddError("Error reading risk limit", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, riskLimitModelFromResponse(current))...)
}

func (r *riskLimitResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	r.upsert(ctx, req.Plan, &resp.State, &resp.Diagnostics, "Error updating risk limit")
}

func (r *riskLimitResource) Delete(_ context.Context, _ resource.DeleteRequest, _ *resource.DeleteResponse) {
	// API に削除エンドポイントがないため、state からのみ除去する。
}

func (r *riskLimitResource) ImportState(ctx context.Context, req resource.ImportStateRequest, resp *resource.ImportStateResponse) {
	if req.ID != riskLimitResourceID {
		resp.Diagnostics.AddError("Invalid Import ID", fmt.Sprintf("Risk limit import ID must be %q.", riskLimitResourceID))
		return
	}
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("id"), riskLimitResourceID)...)
}

func (r *riskLimitResource) configuredClient(diagnostics *diag.Diagnostics) (*traderapi.Client, bool) {
	if r.client == nil {
		diagnostics.AddError("Provider Not Configured", "The t-rader provider must be configured before managing account risk limits.")
		return nil, false
	}
	return r.client, true
}

func (r *riskLimitResource) upsert(ctx context.Context, plan tfsdk.Plan, state *tfsdk.State, diagnostics *diag.Diagnostics, errorSummary string) {
	var planModel riskLimitModel
	diagnostics.Append(plan.Get(ctx, &planModel)...)
	if diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(diagnostics)
	if !ok {
		return
	}

	updated, err := client.PutRiskLimit(ctx, traderapi.PutAccountRiskPolicyRequest{
		MaxSectorRatio: float64AttributeNullable(planModel.MaxSectorRatio),
	})
	if err != nil {
		diagnostics.AddError(errorSummary, err.Error())
		return
	}
	diagnostics.Append(state.Set(ctx, riskLimitModelFromResponse(updated))...)
}

func riskLimitModelFromResponse(response traderapi.AccountRiskPolicyResponse) riskLimitModel {
	return riskLimitModel{
		ID:             types.StringValue(riskLimitResourceID),
		MaxSectorRatio: float64NullableAttribute(response.MaxSectorRatio),
	}
}

func float64AttributeNullable(value types.Float64) nullable.Nullable[float64] {
	if value.IsUnknown() {
		return nullable.Nullable[float64]{}
	}
	if value.IsNull() {
		return nullable.NewNullNullable[float64]()
	}
	return nullable.NewNullableWithValue(value.ValueFloat64())
}

func float64NullableAttribute(value nullable.Nullable[float64]) types.Float64 {
	if !value.IsSpecified() || value.IsNull() {
		return types.Float64Null()
	}
	return types.Float64Value(value.GetOrEmpty())
}

type maxSectorRatioValidator struct{}

func (maxSectorRatioValidator) Description(context.Context) string {
	return "比率は 0 より大きく 1 以下である必要があります。"
}

func (v maxSectorRatioValidator) MarkdownDescription(ctx context.Context) string {
	return v.Description(ctx)
}

func (maxSectorRatioValidator) ValidateFloat64(_ context.Context, req validator.Float64Request, resp *validator.Float64Response) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	value := req.ConfigValue.ValueFloat64()
	if value <= 0 || value > 1 {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid max sector ratio", "max_sector_ratio must be greater than 0 and less than or equal to 1.")
	}
}
