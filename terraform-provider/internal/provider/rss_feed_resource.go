package provider

import (
	"context"
	"errors"
	"fmt"
	"net/url"
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
				MarkdownDescription: "バックエンドが発行するフィード UUID。",
			},
			"source": schema.StringAttribute{
				Required:            true,
				Validators:          []validator.String{rssFeedSourceValidator{}},
				PlanModifiers:       []planmodifier.String{stringplanmodifier.RequiresReplace()},
				MarkdownDescription: "フィードの一意なキー。変更時はフィードを置換します。",
			},
			"display_name": schema.StringAttribute{
				Required:            true,
				Validators:          []validator.String{rssFeedTrimmedStringValidator{attributeName: "display_name"}},
				MarkdownDescription: "画面に表示するフィード名。",
			},
			"url": schema.StringAttribute{
				Required:            true,
				Validators:          []validator.String{rssFeedURLValidator{}},
				MarkdownDescription: "RSS フィードの HTTP または HTTPS URL。",
			},
			"enabled": schema.BoolAttribute{
				Optional:            true,
				Computed:            true,
				MarkdownDescription: "フィードを取得対象にするかどうか。省略時は API の既定値を使用します。",
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
		URL:         plan.URL.ValueString(),
		Enabled:     rssFeedBoolAttributePointer(plan.Enabled),
	})
	if err != nil {
		resp.Diagnostics.AddError("Error creating RSS feed", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, rssFeedModelFromAPI(created))...)
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
	resp.Diagnostics.Append(resp.State.Set(ctx, rssFeedModelFromAPI(feed))...)
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
		DisplayName: stringAttributePointer(plan.DisplayName),
		URL:         stringAttributePointer(plan.URL),
		Enabled:     rssFeedBoolAttributePointer(plan.Enabled),
	})
	if errors.Is(err, traderapi.ErrNotFound) {
		resp.Diagnostics.AddError("Error updating RSS feed", "RSS feed not found.")
		return
	}
	if err != nil {
		resp.Diagnostics.AddError("Error updating RSS feed", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, rssFeedModelFromAPI(updated))...)
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

func rssFeedModelFromAPI(feed traderapi.RssFeed) rssFeedModel {
	return rssFeedModel{
		ID:          types.StringValue(feed.ID),
		Source:      types.StringValue(feed.Source),
		DisplayName: types.StringValue(feed.DisplayName),
		URL:         types.StringValue(feed.URL),
		Enabled:     types.BoolValue(feed.Enabled),
		CreatedAt:   types.StringValue(feed.CreatedAt),
		UpdatedAt:   types.StringValue(feed.UpdatedAt),
	}
}

func rssFeedBoolAttributePointer(value types.Bool) *bool {
	if value.IsNull() || value.IsUnknown() {
		return nil
	}
	result := value.ValueBool()
	return &result
}

type rssFeedSourceValidator struct{}

func (rssFeedSourceValidator) Description(context.Context) string {
	return "must contain only lowercase ASCII letters, numbers, underscores, or hyphens"
}

func (rssFeedSourceValidator) MarkdownDescription(ctx context.Context) string {
	return (rssFeedSourceValidator{}).Description(ctx)
}

func (rssFeedSourceValidator) ValidateString(_ context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	value := req.ConfigValue.ValueString()
	if value == "" || strings.TrimSpace(value) != value {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid RSS feed source", "The source must be non-empty and cannot start or end with whitespace.")
		return
	}
	for _, char := range value {
		if !(char >= 'a' && char <= 'z') && !(char >= '0' && char <= '9') && char != '_' && char != '-' {
			resp.Diagnostics.AddAttributeError(req.Path, "Invalid RSS feed source", "The source can contain only lowercase ASCII letters, numbers, underscores, and hyphens.")
			return
		}
	}
}

type rssFeedTrimmedStringValidator struct {
	attributeName string
}

func (v rssFeedTrimmedStringValidator) Description(context.Context) string {
	return "must be non-empty and have no leading or trailing whitespace"
}

func (v rssFeedTrimmedStringValidator) MarkdownDescription(ctx context.Context) string {
	return v.Description(ctx)
}

func (v rssFeedTrimmedStringValidator) ValidateString(_ context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	value := req.ConfigValue.ValueString()
	if value == "" || strings.TrimSpace(value) != value {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid RSS feed "+v.attributeName, "The value must be non-empty and cannot start or end with whitespace.")
	}
}

type rssFeedURLValidator struct{}

func (rssFeedURLValidator) Description(context.Context) string {
	return "must be a valid HTTP or HTTPS URL without leading or trailing whitespace"
}

func (rssFeedURLValidator) MarkdownDescription(ctx context.Context) string {
	return (rssFeedURLValidator{}).Description(ctx)
}

func (rssFeedURLValidator) ValidateString(_ context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	value := req.ConfigValue.ValueString()
	if value == "" || strings.TrimSpace(value) != value {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid RSS feed URL", "The URL must be non-empty and cannot start or end with whitespace.")
		return
	}
	parsedURL, err := url.Parse(value)
	if err != nil || parsedURL.Host == "" || (parsedURL.Scheme != "http" && parsedURL.Scheme != "https") {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid RSS feed URL", "The URL must be a valid HTTP or HTTPS URL.")
	}
}
