package provider

import (
	"context"
	"errors"
	"fmt"

	"github.com/hashicorp/terraform-plugin-framework/diag"
	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/stringplanmodifier"
	"github.com/hashicorp/terraform-plugin-framework/types"
	"github.com/oapi-codegen/nullable"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
	traderapigen "github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

var (
	_ resource.Resource                = (*noteKindResource)(nil)
	_ resource.ResourceWithImportState = (*noteKindResource)(nil)
)

type noteKindResource struct {
	client *traderapi.Client
}

type noteKindModel struct {
	Key              types.String `tfsdk:"key"`
	DisplayName      types.String `tfsdk:"display_name"`
	RequiresApproval types.Bool   `tfsdk:"requires_approval"`
	Description      types.String `tfsdk:"description"`
	SortOrder        types.Int32  `tfsdk:"sort_order"`
}

func NewNoteKindResource() resource.Resource {
	return &noteKindResource{}
}

func (r *noteKindResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_note_kind"
}

func (r *noteKindResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		MarkdownDescription: "t-rader のノート種別を管理します。",
		Attributes: map[string]schema.Attribute{
			"key": schema.StringAttribute{
				Required:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.RequiresReplace()},
				MarkdownDescription: "ノート種別を識別する key。変更するとリソースを再作成します。import 時は key を指定します。",
			},
			"display_name": schema.StringAttribute{
				Required:            true,
				MarkdownDescription: "UI に表示する名前。",
			},
			"requires_approval": schema.BoolAttribute{
				Required:            true,
				MarkdownDescription: "この種別のノートに承認を必要とするかどうか。",
			},
			"description": schema.StringAttribute{
				Optional:            true,
				MarkdownDescription: "ノート種別の説明。更新時に設定から外すと説明を削除します。",
			},
			"sort_order": schema.Int32Attribute{
				Optional:            true,
				Computed:            true,
				MarkdownDescription: "一覧表示順。省略時は API の現在値を保持し、新規作成時は API の既定値を使用します。",
			},
		},
	}
}

func (r *noteKindResource) Configure(_ context.Context, req resource.ConfigureRequest, resp *resource.ConfigureResponse) {
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

func (r *noteKindResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan noteKindModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	created, err := client.CreateNoteKind(ctx, traderapigen.CreateNoteKindRequest{
		Description:      stringAttributeNullable(plan.Description),
		DisplayName:      plan.DisplayName.ValueString(),
		Key:              plan.Key.ValueString(),
		RequiresApproval: plan.RequiresApproval.ValueBool(),
		SortOrder:        int32AttributeNullable(plan.SortOrder),
	})
	if err != nil {
		resp.Diagnostics.AddError("Error creating note kind", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromNoteKind(created))...)
}

func (r *noteKindResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state noteKindModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	noteKinds, err := client.ListNoteKinds(ctx)
	if err != nil {
		resp.Diagnostics.AddError("Error reading note kind", err.Error())
		return
	}
	for _, noteKind := range noteKinds {
		if noteKind.Key == state.Key.ValueString() {
			resp.Diagnostics.Append(resp.State.Set(ctx, modelFromNoteKind(noteKind))...)
			return
		}
	}
	resp.State.RemoveResource(ctx)
}

func (r *noteKindResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan noteKindModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	updated, err := client.UpdateNoteKind(ctx, plan.Key.ValueString(), traderapigen.UpdateNoteKindRequest{
		Description:      stringAttributeUpdateNullable(plan.Description),
		DisplayName:      nullable.NewNullableWithValue(plan.DisplayName.ValueString()),
		RequiresApproval: nullable.NewNullableWithValue(plan.RequiresApproval.ValueBool()),
		SortOrder:        int32AttributeNullable(plan.SortOrder),
	})
	if err != nil {
		resp.Diagnostics.AddError("Error updating note kind", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromNoteKind(updated))...)
}

func (r *noteKindResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state noteKindModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	if err := client.DeleteNoteKind(ctx, state.Key.ValueString()); err != nil && !errors.Is(err, traderapi.ErrNotFound) {
		resp.Diagnostics.AddError("Error deleting note kind", err.Error())
	}
}

func (r *noteKindResource) ImportState(ctx context.Context, req resource.ImportStateRequest, resp *resource.ImportStateResponse) {
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("key"), req.ID)...)
}

func (r *noteKindResource) configuredClient(diagnostics *diag.Diagnostics) (*traderapi.Client, bool) {
	if r.client == nil {
		diagnostics.AddError("Provider Not Configured", "The t-rader provider must be configured before managing note kinds.")
		return nil, false
	}
	return r.client, true
}

func modelFromNoteKind(noteKind traderapigen.NoteKind) noteKindModel {
	return noteKindModel{
		Key:              types.StringValue(noteKind.Key),
		DisplayName:      types.StringValue(noteKind.DisplayName),
		RequiresApproval: types.BoolValue(noteKind.RequiresApproval),
		Description:      stringNullableAttribute(noteKind.Description),
		SortOrder:        types.Int32Value(noteKind.SortOrder),
	}
}
