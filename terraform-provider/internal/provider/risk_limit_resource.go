package provider

import (
	"context"
	"fmt"

	"github.com/hashicorp/terraform-plugin-framework/attr"
	"github.com/hashicorp/terraform-plugin-framework/diag"
	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/listdefault"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/stringplanmodifier"
	"github.com/hashicorp/terraform-plugin-framework/schema/validator"
	"github.com/hashicorp/terraform-plugin-framework/tfsdk"
	"github.com/hashicorp/terraform-plugin-framework/types"

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
	ID             types.String `tfsdk:"id"`
	MaxGroupRatios types.List   `tfsdk:"max_group_ratios"`
}

type riskLimitGroupRatioModel struct {
	Axis  types.String  `tfsdk:"axis"`
	Ratio types.Float64 `tfsdk:"ratio"`
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
			"max_group_ratios": schema.ListNestedAttribute{
				Optional:            true,
				Default:             listdefault.StaticValue(emptyRiskLimitGroupRatios()),
				MarkdownDescription: "分類軸ごとに、グループに適用する保有比率の上限を指定します。空配列にすると上限を解除します。",
				NestedObject: schema.NestedAttributeObject{
					Attributes: map[string]schema.Attribute{
						"axis": schema.StringAttribute{
							Required:            true,
							MarkdownDescription: "上限を適用する分類軸のキー。",
						},
						"ratio": schema.Float64Attribute{
							Required:            true,
							Validators:          []validator.Float64{maxGroupRatioValidator{}},
							MarkdownDescription: "各グループに適用する上限比率。`(0, 1]` の範囲で指定します。",
						},
					},
				},
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
	model, diagnostics := riskLimitModelFromResponse(ctx, current)
	resp.Diagnostics.Append(diagnostics...)
	if resp.Diagnostics.HasError() {
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, model)...)
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

	groupRatios, conversionDiagnostics := riskLimitGroupRatiosForRequest(ctx, planModel.MaxGroupRatios)
	diagnostics.Append(conversionDiagnostics...)
	if diagnostics.HasError() {
		return
	}
	updated, err := client.PutRiskLimit(ctx, traderapi.PutAccountRiskPolicyRequest{
		MaxGroupRatios: groupRatios,
	})
	if err != nil {
		diagnostics.AddError(errorSummary, err.Error())
		return
	}
	model, conversionDiagnostics := riskLimitModelFromResponse(ctx, updated)
	diagnostics.Append(conversionDiagnostics...)
	if diagnostics.HasError() {
		return
	}
	diagnostics.Append(state.Set(ctx, model)...)
}

func riskLimitModelFromResponse(ctx context.Context, response traderapi.AccountRiskPolicyResponse) (riskLimitModel, diag.Diagnostics) {
	groupRatios, diagnostics := maxGroupRatiosAttribute(ctx, response.MaxGroupRatios)
	if diagnostics.HasError() {
		return riskLimitModel{}, diagnostics
	}
	return riskLimitModel{
		ID:             types.StringValue(riskLimitResourceID),
		MaxGroupRatios: groupRatios,
	}, nil
}

func riskLimitGroupRatioObjectType() types.ObjectType {
	return types.ObjectType{AttrTypes: map[string]attr.Type{
		"axis":  types.StringType,
		"ratio": types.Float64Type,
	}}
}

func emptyRiskLimitGroupRatios() types.List {
	return types.ListValueMust(riskLimitGroupRatioObjectType(), []attr.Value{})
}

func riskLimitGroupRatiosForRequest(ctx context.Context, value types.List) ([]traderapi.GroupRatio, diag.Diagnostics) {
	if value.IsNull() {
		return []traderapi.GroupRatio{}, nil
	}
	if value.IsUnknown() {
		var diagnostics diag.Diagnostics
		diagnostics.AddAttributeError(path.Root("max_group_ratios"), "Unknown group ratio limits", "max_group_ratios must be known when applying the risk limit.")
		return nil, diagnostics
	}

	var models []riskLimitGroupRatioModel
	diagnostics := value.ElementsAs(ctx, &models, false)
	if diagnostics.HasError() {
		return nil, diagnostics
	}

	groupRatios := make([]traderapi.GroupRatio, 0, len(models))
	for _, model := range models {
		groupRatios = append(groupRatios, traderapi.GroupRatio{
			Axis:  model.Axis.ValueString(),
			Ratio: model.Ratio.ValueFloat64(),
		})
	}
	return groupRatios, diagnostics
}

func maxGroupRatiosAttribute(ctx context.Context, values []traderapi.GroupRatio) (types.List, diag.Diagnostics) {
	models := make([]riskLimitGroupRatioModel, 0, len(values))
	for _, value := range values {
		models = append(models, riskLimitGroupRatioModel{
			Axis:  types.StringValue(value.Axis),
			Ratio: types.Float64Value(value.Ratio),
		})
	}
	return types.ListValueFrom(ctx, riskLimitGroupRatioObjectType(), models)
}

type maxGroupRatioValidator struct{}

func (maxGroupRatioValidator) Description(context.Context) string {
	return "比率は 0 より大きく 1 以下である必要があります。"
}

func (v maxGroupRatioValidator) MarkdownDescription(ctx context.Context) string {
	return v.Description(ctx)
}

func (maxGroupRatioValidator) ValidateFloat64(_ context.Context, req validator.Float64Request, resp *validator.Float64Response) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	value := req.ConfigValue.ValueFloat64()
	if value <= 0 || value > 1 {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid group ratio", "ratio must be greater than 0 and less than or equal to 1.")
	}
}
