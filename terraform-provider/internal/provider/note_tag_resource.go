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
	_ resource.Resource                = (*noteTagResource)(nil)
	_ resource.ResourceWithImportState = (*noteTagResource)(nil)
)

type noteTagResource struct {
	client *traderapi.Client
}

type noteTagModel struct {
	Key         types.String `tfsdk:"key"`
	DisplayName types.String `tfsdk:"display_name"`
	Description types.String `tfsdk:"description"`
}

func NewNoteTagResource() resource.Resource {
	return &noteTagResource{}
}

func (r *noteTagResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_note_tag"
}

func (r *noteTagResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		MarkdownDescription: "t-rader のノートタグ語彙を管理します。",
		Attributes: map[string]schema.Attribute{
			"key": schema.StringAttribute{
				Required:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.RequiresReplace()},
				MarkdownDescription: "ノートタグを識別する key。変更するとリソースを再作成します。import 時は key を指定します。",
			},
			"display_name": schema.StringAttribute{
				Required:            true,
				MarkdownDescription: "UI に表示する名前。",
			},
			"description": schema.StringAttribute{
				Optional:            true,
				MarkdownDescription: "ノートタグの説明。更新時に設定から外すと説明を削除します。",
			},
		},
	}
}

func (r *noteTagResource) Configure(_ context.Context, req resource.ConfigureRequest, resp *resource.ConfigureResponse) {
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

func (r *noteTagResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan noteTagModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	created, err := client.CreateNoteTag(ctx, traderapigen.CreateNoteTagRequest{
		Description: stringAttributeNullable(plan.Description),
		DisplayName: plan.DisplayName.ValueString(),
		Key:         plan.Key.ValueString(),
	})
	if err != nil {
		resp.Diagnostics.AddError("Error creating note tag", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromNoteTag(created))...)
}

func (r *noteTagResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state noteTagModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	noteTags, err := client.ListNoteTags(ctx)
	if err != nil {
		resp.Diagnostics.AddError("Error reading note tag", err.Error())
		return
	}
	for _, noteTag := range noteTags {
		if noteTag.Key == state.Key.ValueString() {
			resp.Diagnostics.Append(resp.State.Set(ctx, modelFromNoteTag(noteTag))...)
			return
		}
	}
	resp.State.RemoveResource(ctx)
}

func (r *noteTagResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan noteTagModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	updated, err := client.UpdateNoteTag(ctx, plan.Key.ValueString(), traderapigen.UpdateNoteTagRequest{
		Description: stringAttributeUpdateNullable(plan.Description),
		DisplayName: nullable.NewNullableWithValue(plan.DisplayName.ValueString()),
	})
	if err != nil {
		resp.Diagnostics.AddError("Error updating note tag", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromNoteTag(updated))...)
}

func (r *noteTagResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state noteTagModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	if err := client.DeleteNoteTag(ctx, state.Key.ValueString()); err != nil && !errors.Is(err, traderapi.ErrNotFound) {
		resp.Diagnostics.AddError("Error deleting note tag", err.Error())
	}
}

func (r *noteTagResource) ImportState(ctx context.Context, req resource.ImportStateRequest, resp *resource.ImportStateResponse) {
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("key"), req.ID)...)
}

func (r *noteTagResource) configuredClient(diagnostics *diag.Diagnostics) (*traderapi.Client, bool) {
	if r.client == nil {
		diagnostics.AddError("Provider Not Configured", "The t-rader provider must be configured before managing note tags.")
		return nil, false
	}
	return r.client, true
}

func modelFromNoteTag(noteTag traderapigen.NoteTag) noteTagModel {
	return noteTagModel{
		Key:         types.StringValue(noteTag.Key),
		DisplayName: types.StringValue(noteTag.DisplayName),
		Description: stringNullableAttribute(noteTag.Description),
	}
}
