package provider

import (
	"context"
	"errors"
	"fmt"
	"net/url"
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
	_ resource.Resource                = (*rssFeedResource)(nil)
	_ resource.ResourceWithImportState = (*rssFeedResource)(nil)
)

type rssFeedResource struct {
	client *traderapi.Client
}

type rssFeedModel struct {
	ID          types.String `tfsdk:"id"`
	Source      types.String `tfsdk:"source"`
	DisplayName types.String `tfsdk:"display_name"`
	URL         types.String `tfsdk:"url"`
	Enabled     types.Bool   `tfsdk:"enabled"`
	CreatedAt   types.String `tfsdk:"created_at"`
	UpdatedAt   types.String `tfsdk:"updated_at"`
}

func NewRssFeedResource() resource.Resource {
	return &rssFeedResource{}
}

func (r *rssFeedResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_rss_feed"
}

func (r *rssFeedResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		MarkdownDescription: "t-rader の RSS フィードを管理します。",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Computed:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.UseStateForUnknown()},
				MarkdownDescription: "バックエンドが発行する RSS フィード UUID。",
			},
			"source": schema.StringAttribute{
				Required:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.RequiresReplace()},
				Validators:          []validator.String{rssFeedSourceValidator{}},
				MarkdownDescription: "一意な machine key。変更するとフィードを再作成します。",
			},
			"display_name": schema.StringAttribute{
				Required:            true,
				Validators:          []validator.String{rssFeedDisplayNameValidator{}},
				MarkdownDescription: "UI に表示する名前。",
			},
			"url": schema.StringAttribute{
				Required:            true,
				Validators:          []validator.String{rssFeedURLValidator{}},
				MarkdownDescription: "RSS フィードの HTTP または HTTPS URL。",
			},
			"enabled": schema.BoolAttribute{
				Optional:            true,
				Computed:            true,
				MarkdownDescription: "取得対象にするかどうか。省略時は API の既定値を使用します。",
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

func (r *rssFeedResource) Configure(_ context.Context, req resource.ConfigureRequest, resp *resource.ConfigureResponse) {
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

func (r *rssFeedResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan rssFeedModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	created, err := client.CreateRssFeed(ctx, traderapi.CreateRssFeedRequest{
		Source:      plan.Source.ValueString(),
		DisplayName: plan.DisplayName.ValueString(),
		Url:         plan.URL.ValueString(),
		Enabled:     boolAttributeNullable(plan.Enabled),
	})
	if err != nil {
		resp.Diagnostics.AddError("Error creating RSS feed", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromRssFeed(created))...)
}

func (r *rssFeedResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state rssFeedModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	feed, err := client.GetRssFeed(ctx, state.ID.ValueString())
	if errors.Is(err, traderapi.ErrNotFound) {
		resp.State.RemoveResource(ctx)
		return
	}
	if err != nil {
		resp.Diagnostics.AddError("Error reading RSS feed", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromRssFeed(feed))...)
}

func (r *rssFeedResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan rssFeedModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	var state rssFeedModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	updated, err := client.UpdateRssFeed(ctx, state.ID.ValueString(), traderapi.UpdateRssFeedRequest{
		DisplayName: stringAttributeUpdateNullable(plan.DisplayName),
		Url:         stringAttributeUpdateNullable(plan.URL),
		Enabled:     boolAttributeUpdateNullable(plan.Enabled),
	})
	if err != nil {
		resp.Diagnostics.AddError("Error updating RSS feed", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromRssFeed(updated))...)
}

func (r *rssFeedResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state rssFeedModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	if err := client.DeleteRssFeed(ctx, state.ID.ValueString()); err != nil && !errors.Is(err, traderapi.ErrNotFound) {
		resp.Diagnostics.AddError("Error deleting RSS feed", err.Error())
	}
}

func (r *rssFeedResource) ImportState(ctx context.Context, req resource.ImportStateRequest, resp *resource.ImportStateResponse) {
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("id"), req.ID)...)
}

func (r *rssFeedResource) configuredClient(diagnostics *diag.Diagnostics) (*traderapi.Client, bool) {
	if r.client == nil {
		diagnostics.AddError("Provider Not Configured", "The t-rader provider must be configured before managing RSS feeds.")
		return nil, false
	}
	return r.client, true
}

func modelFromRssFeed(feed traderapi.RssFeed) rssFeedModel {
	return rssFeedModel{
		ID:          types.StringValue(feed.Id.String()),
		Source:      types.StringValue(feed.Source),
		DisplayName: types.StringValue(feed.DisplayName),
		URL:         types.StringValue(feed.Url),
		Enabled:     types.BoolValue(feed.Enabled),
		CreatedAt:   types.StringValue(feed.CreatedAt.Format(time.RFC3339Nano)),
		UpdatedAt:   types.StringValue(feed.UpdatedAt.Format(time.RFC3339Nano)),
	}
}

func boolAttributeNullable(value types.Bool) nullable.Nullable[bool] {
	if value.IsNull() || value.IsUnknown() {
		return nullable.Nullable[bool]{}
	}
	return nullable.NewNullableWithValue(value.ValueBool())
}

func boolAttributeUpdateNullable(value types.Bool) nullable.Nullable[bool] {
	if value.IsNull() || value.IsUnknown() {
		return nullable.Nullable[bool]{}
	}
	return nullable.NewNullableWithValue(value.ValueBool())
}

type rssFeedSourceValidator struct{}

func (rssFeedSourceValidator) Description(_ context.Context) string {
	return "source は小文字英数字、ハイフン、アンダースコアで指定してください。"
}

func (rssFeedSourceValidator) MarkdownDescription(ctx context.Context) string {
	return rssFeedSourceValidator{}.Description(ctx)
}

func (rssFeedSourceValidator) ValidateString(ctx context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	value := req.ConfigValue.ValueString()
	if value == "" || value != strings.TrimSpace(value) || strings.IndexFunc(value, func(char rune) bool {
		return !(char >= 'a' && char <= 'z') && !(char >= '0' && char <= '9') && char != '_' && char != '-'
	}) >= 0 {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid RSS feed source", "Source must contain only lowercase letters, digits, hyphens, and underscores without surrounding whitespace.")
	}
}

type rssFeedDisplayNameValidator struct{}

func (rssFeedDisplayNameValidator) Description(_ context.Context) string {
	return "表示名は空にできず、前後に空白を含められません。"
}

func (rssFeedDisplayNameValidator) MarkdownDescription(ctx context.Context) string {
	return rssFeedDisplayNameValidator{}.Description(ctx)
}

func (rssFeedDisplayNameValidator) ValidateString(ctx context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	value := req.ConfigValue.ValueString()
	if strings.TrimSpace(value) == "" || value != strings.TrimSpace(value) {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid RSS feed display name", "Display name must not be empty or have surrounding whitespace.")
	}
}

type rssFeedURLValidator struct{}

func (rssFeedURLValidator) Description(_ context.Context) string {
	return "URL は HTTP または HTTPS で指定し、前後に空白を含めないでください。"
}

func (rssFeedURLValidator) MarkdownDescription(ctx context.Context) string {
	return rssFeedURLValidator{}.Description(ctx)
}

func (rssFeedURLValidator) ValidateString(ctx context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	value := req.ConfigValue.ValueString()
	parsed, err := url.Parse(value)
	if err != nil || value != strings.TrimSpace(value) || parsed.Host == "" || (parsed.Scheme != "http" && parsed.Scheme != "https") {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid RSS feed URL", "URL must be an absolute HTTP or HTTPS URL without surrounding whitespace.")
	}
}
