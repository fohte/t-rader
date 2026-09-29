package provider

import (
	"context"
	"fmt"
	"os"

	"github.com/hashicorp/terraform-plugin-framework/datasource"
	"github.com/hashicorp/terraform-plugin-framework/provider"
	"github.com/hashicorp/terraform-plugin-framework/provider/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/types"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
)

const ProviderAddress = "fohte.net/fohte/trader"

var _ provider.Provider = (*traderProvider)(nil)

type traderProvider struct{}

type providerModel struct {
	BaseURL      types.String `tfsdk:"base_url"`
	ClientID     types.String `tfsdk:"client_id"`
	ClientSecret types.String `tfsdk:"client_secret"`
}

func New() provider.Provider {
	return &traderProvider{}
}

func (p *traderProvider) Metadata(_ context.Context, _ provider.MetadataRequest, resp *provider.MetadataResponse) {
	resp.TypeName = "trader"
	resp.Version = "0.0.0"
}

func (p *traderProvider) Schema(_ context.Context, _ provider.SchemaRequest, resp *provider.SchemaResponse) {
	resp.Schema = schema.Schema{
		Attributes: map[string]schema.Attribute{
			"base_url": schema.StringAttribute{
				Optional:            true,
				MarkdownDescription: "t-rader API の base URL。省略時は `TRADER_BASE_URL` 環境変数を使用します。",
			},
			"client_id": schema.StringAttribute{
				Optional:            true,
				Sensitive:           true,
				MarkdownDescription: "Cloudflare Access service token の client ID。省略時は `TRADER_CLIENT_ID` 環境変数を使用します。",
			},
			"client_secret": schema.StringAttribute{
				Optional:            true,
				Sensitive:           true,
				MarkdownDescription: "Cloudflare Access service token の client secret。省略時は `TRADER_CLIENT_SECRET` 環境変数を使用します。",
			},
		},
	}
}

func (p *traderProvider) Configure(ctx context.Context, req provider.ConfigureRequest, resp *provider.ConfigureResponse) {
	var config providerModel
	resp.Diagnostics.Append(req.Config.Get(ctx, &config)...)
	if resp.Diagnostics.HasError() {
		return
	}

	baseURL, err := configuredValue(config.BaseURL, "base_url", "TRADER_BASE_URL")
	if err != nil {
		resp.Diagnostics.AddError("Invalid provider configuration", err.Error())
		return
	}
	if baseURL == "" {
		resp.Diagnostics.AddError("Missing API base URL", "Set `base_url` or the `TRADER_BASE_URL` environment variable.")
		return
	}
	clientID, err := configuredValue(config.ClientID, "client_id", "TRADER_CLIENT_ID")
	if err != nil {
		resp.Diagnostics.AddError("Invalid provider configuration", err.Error())
		return
	}
	clientSecret, err := configuredValue(config.ClientSecret, "client_secret", "TRADER_CLIENT_SECRET")
	if err != nil {
		resp.Diagnostics.AddError("Invalid provider configuration", err.Error())
		return
	}
	if (clientID == "") != (clientSecret == "") {
		resp.Diagnostics.AddError("Incomplete Cloudflare Access credentials", "Set both `client_id` and `client_secret`, or omit both when connecting to a backend without Cloudflare Access.")
		return
	}

	client, err := traderapi.New(baseURL, clientID, clientSecret)
	if err != nil {
		resp.Diagnostics.AddError("Invalid API base URL", err.Error())
		return
	}
	if err := client.CheckConnection(ctx); err != nil {
		resp.Diagnostics.AddError("Unable to connect to t-rader API", fmt.Sprintf("GET /api/strategies failed: %s", err))
		return
	}
	resp.ResourceData = client
}

func (p *traderProvider) Resources(_ context.Context) []func() resource.Resource {
	return []func() resource.Resource{NewStrategyResource, NewRiskLimitResource, NewCustomIndicatorResource}
}

func (p *traderProvider) DataSources(_ context.Context) []func() datasource.DataSource {
	return nil
}

func configuredValue(value types.String, attributeName, environmentVariable string) (string, error) {
	if value.IsUnknown() {
		return "", fmt.Errorf("%s must be known during provider configuration", attributeName)
	}
	if !value.IsNull() {
		return value.ValueString(), nil
	}
	return os.Getenv(environmentVariable), nil
}
