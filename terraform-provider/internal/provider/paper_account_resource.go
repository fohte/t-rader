package provider

import (
	"context"
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
	openapi_types "github.com/oapi-codegen/runtime/types"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
	traderapigen "github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

var (
	_ resource.Resource                = (*paperAccountResource)(nil)
	_ resource.ResourceWithImportState = (*paperAccountResource)(nil)
)

type paperAccountResource struct {
	client *traderapi.Client
}

type paperAccountModel struct {
	ID               types.String  `tfsdk:"id"`
	StrategyID       types.String  `tfsdk:"strategy_id"`
	Purpose          types.String  `tfsdk:"purpose"`
	Name             types.String  `tfsdk:"name"`
	InitialCashJpy   types.Float64 `tfsdk:"initial_cash_jpy"`
	BenchmarkStockID types.String  `tfsdk:"benchmark_stock_id"`
	StartedOn        types.String  `tfsdk:"started_on"`
}

func NewPaperAccountResource() resource.Resource {
	return &paperAccountResource{}
}

func (r *paperAccountResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_paper_account"
}

func (r *paperAccountResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		MarkdownDescription: "ペーパートレード口座を作成します。作成後に属性は変更できません。変更を適用するとエラーになります。\n\n" +
			"`terraform destroy` は Terraform state から口座を外しますが、バックエンドの口座は削除しません。再度管理する場合は口座 UUID を使って import してください。\n\n" +
			"```terraform\n" +
			"resource \"trader_paper_account\" \"example\" {\n" +
			"  strategy_id       = \"00000000-0000-4000-8000-000000000301\"\n" +
			"  purpose           = \"sample\"\n" +
			"  name              = \"sample-paper-account\"\n" +
			"  initial_cash_jpy  = 1000000\n" +
			"  benchmark_stock_id = \"synthetic-stock-id\"\n" +
			"  started_on        = \"2026-01-02\"\n" +
			"}\n" +
			"```",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Computed:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.UseStateForUnknown()},
				MarkdownDescription: "バックエンドが発行する口座 UUID。import 時に指定します。",
			},
			"strategy_id": schema.StringAttribute{
				Required:            true,
				Validators:          []validator.String{paperAccountStrategyIDValidator{}},
				MarkdownDescription: "口座を作成する戦略 UUID。作成後は変更できません。",
			},
			"purpose": schema.StringAttribute{
				Required:            true,
				Validators:          []validator.String{paperAccountTrimmedStringValidator{attribute: "purpose"}},
				MarkdownDescription: "口座に紐づける agent 設定の purpose。作成後は変更できません。",
			},
			"name": schema.StringAttribute{
				Required:            true,
				Validators:          []validator.String{paperAccountTrimmedStringValidator{attribute: "name"}},
				MarkdownDescription: "口座名。作成後は変更できません。",
			},
			"initial_cash_jpy": schema.Float64Attribute{
				Required:            true,
				MarkdownDescription: "口座の初期資金 (円)。作成後は変更できません。",
			},
			"benchmark_stock_id": schema.StringAttribute{
				Optional:            true,
				MarkdownDescription: "成績比較に使う銘柄 ID。省略できます。作成後は変更できません。",
			},
			"started_on": schema.StringAttribute{
				Required:            true,
				MarkdownDescription: "口座の開始日 (`YYYY-MM-DD`)。作成後は変更できません。",
			},
		},
	}
}

func (r *paperAccountResource) Configure(_ context.Context, req resource.ConfigureRequest, resp *resource.ConfigureResponse) {
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

func (r *paperAccountResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan paperAccountModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	strategyID, err := uuid.Parse(plan.StrategyID.ValueString())
	if err != nil {
		resp.Diagnostics.AddAttributeError(path.Root("strategy_id"), "Invalid strategy_id", "strategy_id は UUID にしてください。")
		return
	}
	if strategyID.String() != plan.StrategyID.ValueString() {
		resp.Diagnostics.AddAttributeError(path.Root("strategy_id"), "Non-canonical strategy_id", "strategy_id は小文字の正規 UUID にしてください。")
		return
	}
	startedOn, err := time.Parse("2006-01-02", plan.StartedOn.ValueString())
	if err != nil {
		resp.Diagnostics.AddAttributeError(path.Root("started_on"), "Invalid started_on", "started_on は YYYY-MM-DD 形式にしてください。")
		return
	}

	created, err := client.CreatePaperAccount(ctx, traderapigen.CreatePaperAccountRequest{
		BenchmarkStockId: stringAttributeNullable(plan.BenchmarkStockID),
		InitialCashJpy:   plan.InitialCashJpy.ValueFloat64(),
		Name:             plan.Name.ValueString(),
		Purpose:          plan.Purpose.ValueString(),
		StartedOn:        openapi_types.Date{Time: startedOn},
		StrategyId:       strategyID,
	})
	if err != nil {
		resp.Diagnostics.AddError("Error creating paper account", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, modelFromPaperAccount(created))...)
}

func (r *paperAccountResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state paperAccountModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	accounts, err := client.ListPaperAccounts(ctx)
	if err != nil {
		resp.Diagnostics.AddError("Error reading paper account", err.Error())
		return
	}
	for _, account := range accounts {
		if account.Id.String() == state.ID.ValueString() {
			resp.Diagnostics.Append(resp.State.Set(ctx, modelFromPaperAccount(account))...)
			return
		}
	}
	resp.State.RemoveResource(ctx)
}

func (r *paperAccountResource) Update(_ context.Context, _ resource.UpdateRequest, resp *resource.UpdateResponse) {
	resp.Diagnostics.AddError("Paper account attributes are immutable", "作成後の口座属性は変更できません。設定を元に戻すか、Terraform 管理から外す場合はリソースを構成から削除して `terraform state rm` を実行してください。")
}

func (r *paperAccountResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state paperAccountModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	resp.Diagnostics.AddWarning(
		"Paper account remains in the backend",
		fmt.Sprintf("Terraform state から口座 %s (%s) を外しますが、バックエンドの口座は削除されません。", state.Name.ValueString(), state.ID.ValueString()),
	)
}

func (r *paperAccountResource) ImportState(ctx context.Context, req resource.ImportStateRequest, resp *resource.ImportStateResponse) {
	accountID, err := uuid.Parse(req.ID)
	if err != nil {
		resp.Diagnostics.AddError("Invalid Import ID", "Import ID must be a paper account UUID.")
		return
	}
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("id"), accountID.String())...)
}

func (r *paperAccountResource) configuredClient(diagnostics *diag.Diagnostics) (*traderapi.Client, bool) {
	if r.client == nil {
		diagnostics.AddError("Provider Not Configured", "The t-rader provider must be configured before managing paper accounts.")
		return nil, false
	}
	return r.client, true
}

func modelFromPaperAccount(account traderapigen.PaperAccount) paperAccountModel {
	return paperAccountModel{
		ID:               types.StringValue(account.Id.String()),
		StrategyID:       types.StringValue(account.StrategyId.String()),
		Purpose:          types.StringValue(account.Purpose),
		Name:             types.StringValue(account.Name),
		InitialCashJpy:   types.Float64Value(account.InitialCashJpy),
		BenchmarkStockID: stringNullableAttribute(account.BenchmarkStockId),
		StartedOn:        types.StringValue(account.StartedOn.String()),
	}
}

type paperAccountStrategyIDValidator struct{}

func (paperAccountStrategyIDValidator) Description(context.Context) string {
	return "strategy_id は小文字の正規 UUID にしてください。"
}

func (v paperAccountStrategyIDValidator) MarkdownDescription(ctx context.Context) string {
	return v.Description(ctx)
}

func (paperAccountStrategyIDValidator) ValidateString(_ context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	value := req.ConfigValue.ValueString()
	parsedID, err := uuid.Parse(value)
	if err != nil {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid strategy_id", "strategy_id は UUID にしてください。")
		return
	}
	if parsedID.String() != value {
		resp.Diagnostics.AddAttributeError(req.Path, "Non-canonical strategy_id", "strategy_id は小文字の正規 UUID にしてください。")
	}
}

type paperAccountTrimmedStringValidator struct {
	attribute string
}

func (v paperAccountTrimmedStringValidator) Description(context.Context) string {
	return fmt.Sprintf("%s は空白のみ、または前後に空白を含む値にできません。", v.attribute)
}

func (v paperAccountTrimmedStringValidator) MarkdownDescription(ctx context.Context) string {
	return v.Description(ctx)
}

func (v paperAccountTrimmedStringValidator) ValidateString(_ context.Context, req validator.StringRequest, resp *validator.StringResponse) {
	if req.ConfigValue.IsNull() || req.ConfigValue.IsUnknown() {
		return
	}
	value := req.ConfigValue.ValueString()
	if strings.TrimSpace(value) == "" {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid paper account "+v.attribute, v.attribute+" は空にできません。")
		return
	}
	if strings.TrimSpace(value) != value {
		resp.Diagnostics.AddAttributeError(req.Path, "Invalid paper account "+v.attribute, v.attribute+" の前後に空白を指定できません。")
	}
}
