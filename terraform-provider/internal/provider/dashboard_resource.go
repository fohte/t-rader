package provider

import (
	"context"
	"errors"
	"fmt"
	"strings"

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
	_ resource.Resource                = (*dashboardResource)(nil)
	_ resource.ResourceWithImportState = (*dashboardResource)(nil)
)

type dashboardResource struct {
	client *traderapi.Client
}

type dashboardModel struct {
	Key    types.String `tfsdk:"key"`
	Config types.String `tfsdk:"config"`
}

func NewDashboardResource() resource.Resource {
	return &dashboardResource{}
}

func (r *dashboardResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_dashboard"
}

func (r *dashboardResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		MarkdownDescription: "ダッシュボードを管理します。`config` には YAML を指定できます。",
		Attributes: map[string]schema.Attribute{
			"key": schema.StringAttribute{
				Required:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.RequiresReplace()},
				Validators:          []validator.String{dashboardKeyValidator{}},
				MarkdownDescription: "ダッシュボードを識別する key。変更するとダッシュボードを再作成します。",
			},
			"config": schema.StringAttribute{
				Required:            true,
				MarkdownDescription: "ダッシュボード設定を表す YAML 文字列。Terraform の `file()` で読み込めます。",
			},
		},
	}
}

func (r *dashboardResource) Configure(_ context.Context, req resource.ConfigureRequest, resp *resource.ConfigureResponse) {
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

func (r *dashboardResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan dashboardModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	created, err := client.CreateDashboard(ctx, traderapigen.CreateDashboardRequest{
		Key:    plan.Key.ValueString(),
		Config: plan.Config.ValueString(),
	})
	if err != nil {
		resp.Diagnostics.AddError("Error creating dashboard", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromDashboard(created))...)
}

func (r *dashboardResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state dashboardModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	dashboard, err := client.GetDashboard(ctx, state.Key.ValueString())
	if errors.Is(err, traderapi.ErrNotFound) {
		resp.State.RemoveResource(ctx)
		return
	}
	if err != nil {
		resp.Diagnostics.AddError("Error reading dashboard", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromDashboard(dashboard))...)
}

func (r *dashboardResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan dashboardModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	var state dashboardModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	updated, err := client.UpdateDashboard(ctx, state.Key.ValueString(), traderapigen.UpdateDashboardRequest{
		Config: plan.Config.ValueString(),
	})
	if err != nil {
		if errors.Is(err, traderapi.ErrNotFound) {
			resp.Diagnostics.AddError("Error updating dashboard", "The dashboard no longer exists.")
			return
		}
		resp.Diagnostics.AddError("Error updating dashboard", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromDashboard(updated))...)
}

func (r *dashboardResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state dashboardModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}
	if err := client.DeleteDashboard(ctx, state.Key.ValueString()); err != nil && !errors.Is(err, traderapi.ErrNotFound) {
		resp.Diagnostics.AddError("Error deleting dashboard", err.Error())
	}
}

func (r *dashboardResource) ImportState(ctx context.Context, req resource.ImportStateRequest, resp *resource.ImportStateResponse) {
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("key"), req.ID)...)
}

func (r *dashboardResource) configuredClient(diagnostics *diag.Diagnostics) (*traderapi.Client, bool) {
	if r.client == nil {
		diagnostics.AddError("Provider Not Configured", "The t-rader provider must be configured before managing dashboards.")
		return nil, false
	}
	return r.client, true
}

func modelFromDashboard(dashboard traderapigen.Dashboard) dashboardModel {
	return dashboardModel{
		Key:    types.StringValue(dashboard.Key),
		Config: types.StringValue(dashboard.Config),
	}
}

type dashboardKeyValidator struct{}

func (dashboardKeyValidator) Description(context.Context) string {
	return "key は空でなく、前後の空白を含まない値にしてください。"
}

func (v dashboardKeyValidator) MarkdownDescription(ctx context.Context) string {
	return v.Description(ctx)
}

func (dashboardKeyValidator) ValidateString(_ context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	key := req.ConfigValue.ValueString()
	if key == "" || key != strings.TrimSpace(key) {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid dashboard key", "Key must not be empty or have surrounding whitespace.")
	}
}
