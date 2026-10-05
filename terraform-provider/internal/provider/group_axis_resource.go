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
	_ resource.Resource                = (*groupAxisResource)(nil)
	_ resource.ResourceWithImportState = (*groupAxisResource)(nil)
)

type groupAxisResource struct {
	client *traderapi.Client
}

type groupAxisModel struct {
	Key         types.String `tfsdk:"key"`
	Name        types.String `tfsdk:"name"`
	Description types.String `tfsdk:"description"`
	SyncSource  types.String `tfsdk:"sync_source"`
}

func NewGroupAxisResource() resource.Resource {
	return &groupAxisResource{}
}

func (r *groupAxisResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_group_axis"
}

func (r *groupAxisResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		MarkdownDescription: "t-rader の分類軸を管理します。",
		Attributes: map[string]schema.Attribute{
			"key": schema.StringAttribute{
				Required:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.RequiresReplace()},
				Validators:          []validator.String{groupAxisKeyValidator{}},
				MarkdownDescription: "分類軸を識別する key。変更すると分類軸を再作成します。",
			},
			"name": schema.StringAttribute{
				Required:            true,
				MarkdownDescription: "表示名。",
			},
			"description": schema.StringAttribute{
				Required:            true,
				MarkdownDescription: "分類する観点の説明。",
			},
			"sync_source": schema.StringAttribute{
				Optional:            true,
				MarkdownDescription: "分類軸のグループ内容を同期するデータソース。",
			},
		},
	}
}

func (r *groupAxisResource) Configure(_ context.Context, req resource.ConfigureRequest, resp *resource.ConfigureResponse) {
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

func (r *groupAxisResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan groupAxisModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	created, err := client.CreateGroupAxis(ctx, traderapigen.CreateGroupAxisRequest{
		Key:         plan.Key.ValueString(),
		Name:        plan.Name.ValueString(),
		Description: plan.Description.ValueString(),
		SyncSource:  stringAttributeNullable(plan.SyncSource),
	})
	if err != nil {
		resp.Diagnostics.AddError("Error creating group axis", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromGroupAxis(created))...)
}

func (r *groupAxisResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state groupAxisModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	axis, err := client.GetGroupAxis(ctx, state.Key.ValueString())
	if errors.Is(err, traderapi.ErrNotFound) {
		resp.State.RemoveResource(ctx)
		return
	}
	if err != nil {
		resp.Diagnostics.AddError("Error reading group axis", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromGroupAxis(axis))...)
}

func (r *groupAxisResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan groupAxisModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	updated, err := client.UpdateGroupAxis(ctx, plan.Key.ValueString(), traderapigen.UpdateGroupAxisRequest{
		Name:        stringAttributeUpdateNullable(plan.Name),
		Description: stringAttributeUpdateNullable(plan.Description),
		SyncSource:  stringAttributeUpdateNullable(plan.SyncSource),
	})
	if err != nil {
		resp.Diagnostics.AddError("Error updating group axis", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromGroupAxis(updated))...)
}

func (r *groupAxisResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state groupAxisModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	if err := client.DeleteGroupAxis(ctx, state.Key.ValueString()); err != nil && !errors.Is(err, traderapi.ErrNotFound) {
		resp.Diagnostics.AddError("Error deleting group axis", err.Error())
	}
}

func (r *groupAxisResource) ImportState(ctx context.Context, req resource.ImportStateRequest, resp *resource.ImportStateResponse) {
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("key"), req.ID)...)
}

func (r *groupAxisResource) configuredClient(diagnostics *diag.Diagnostics) (*traderapi.Client, bool) {
	if r.client == nil {
		diagnostics.AddError("Provider Not Configured", "The t-rader provider must be configured before managing group axes.")
		return nil, false
	}
	return r.client, true
}

func modelFromGroupAxis(axis traderapigen.GroupAxis) groupAxisModel {
	return groupAxisModel{
		Key:         types.StringValue(axis.Key),
		Name:        types.StringValue(axis.Name),
		Description: types.StringValue(axis.Description),
		SyncSource:  stringNullableAttribute(axis.SyncSource),
	}
}

type groupAxisKeyValidator struct{}

func (groupAxisKeyValidator) Description(context.Context) string {
	return "key は空でなく、'/' や前後の空白を含まない値にしてください。"
}

func (v groupAxisKeyValidator) MarkdownDescription(ctx context.Context) string {
	return v.Description(ctx)
}

func (groupAxisKeyValidator) ValidateString(_ context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	key := req.ConfigValue.ValueString()
	if key == "" || key != strings.TrimSpace(key) || strings.Contains(key, "/") {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid group axis key", "Key must not be empty, contain '/', or have surrounding whitespace.")
	}
}
