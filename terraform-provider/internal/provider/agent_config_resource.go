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
	"github.com/hashicorp/terraform-plugin-framework/types"
	"gopkg.in/yaml.v3"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
)

var (
	_ resource.Resource                   = (*agentConfigResource)(nil)
	_ resource.ResourceWithImportState    = (*agentConfigResource)(nil)
	_ resource.ResourceWithValidateConfig = (*agentConfigResource)(nil)
)

type agentConfigResource struct {
	client *traderapi.Client
}

type agentConfigModel struct {
	Purpose    types.String `tfsdk:"purpose"`
	AgentsMd   types.String `tfsdk:"agents_md"`
	Skills     types.Map    `tfsdk:"skills"`
	AgentGraph types.String `tfsdk:"agent_graph"`
}

func NewAgentConfigResource() resource.Resource {
	return &agentConfigResource{}
}

func (r *agentConfigResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_agent_config"
}

func (r *agentConfigResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		MarkdownDescription: "t-rader の agent 設定を管理します。",
		Attributes: map[string]schema.Attribute{
			"purpose": schema.StringAttribute{
				Required:            true,
				PlanModifiers:       []planmodifier.String{stringplanmodifier.RequiresReplace()},
				MarkdownDescription: "agent 設定を識別する purpose。import 時は purpose を指定します。",
			},
			"agents_md": schema.StringAttribute{
				Required:            true,
				MarkdownDescription: "agent に渡す指示文。",
			},
			"skills": schema.MapAttribute{
				Required:            true,
				ElementType:         types.StringType,
				MarkdownDescription: "skill 名から内容への map。更新時は一覧全体を置き換えます。",
			},
			"agent_graph": schema.StringAttribute{
				Required:            true,
				MarkdownDescription: "agent のフェーズ構成を表す YAML。",
			},
		},
	}
}

func (r *agentConfigResource) Configure(_ context.Context, req resource.ConfigureRequest, resp *resource.ConfigureResponse) {
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

func (r *agentConfigResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan agentConfigModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}
	skills, ok := skillsFromAttribute(ctx, plan.Skills, &resp.Diagnostics)
	if !ok {
		return
	}

	purpose := plan.Purpose.ValueString()
	if err := client.CreateAgentConfig(ctx, purpose); err != nil {
		resp.Diagnostics.AddError("Error creating agent config", err.Error())
		return
	}
	if err := client.UpdateAgentConfig(ctx, purpose, plan.AgentsMd.ValueString(), skills, plan.AgentGraph.ValueString()); err != nil {
		if cleanupErr := client.DeleteAgentConfig(ctx, purpose); cleanupErr != nil {
			err = fmt.Errorf("%w; remove partially created agent config: %v", err, cleanupErr)
		}
		resp.Diagnostics.AddError("Error configuring created agent config", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, plan)...)
}

func (r *agentConfigResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state agentConfigModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	config, err := client.GetAgentConfig(ctx, state.Purpose.ValueString())
	if errors.Is(err, traderapi.ErrAgentConfigNotFound) {
		resp.State.RemoveResource(ctx)
		return
	}
	if err != nil {
		resp.Diagnostics.AddError("Error reading agent config", err.Error())
		return
	}

	skillValues, err := skillsFromConfig(config.Skills)
	if err != nil {
		resp.Diagnostics.AddError("Error reading agent config", err.Error())
		return
	}
	skills, diagnostics := types.MapValueFrom(ctx, types.StringType, skillValues)
	resp.Diagnostics.Append(diagnostics...)
	if resp.Diagnostics.HasError() {
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, agentConfigModel{
		Purpose:    state.Purpose,
		AgentsMd:   types.StringValue(config.AgentsMd),
		Skills:     skills,
		AgentGraph: types.StringValue(config.AgentGraph),
	})...)
}

func (r *agentConfigResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan agentConfigModel
	resp.Diagnostics.Append(req.Plan.Get(ctx, &plan)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}
	skills, ok := skillsFromAttribute(ctx, plan.Skills, &resp.Diagnostics)
	if !ok {
		return
	}

	if err := client.UpdateAgentConfig(ctx, plan.Purpose.ValueString(), plan.AgentsMd.ValueString(), skills, plan.AgentGraph.ValueString()); err != nil {
		resp.Diagnostics.AddError("Error updating agent config", err.Error())
		return
	}
	resp.Diagnostics.Append(resp.State.Set(ctx, plan)...)
}

func (r *agentConfigResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state agentConfigModel
	resp.Diagnostics.Append(req.State.Get(ctx, &state)...)
	if resp.Diagnostics.HasError() {
		return
	}
	client, ok := r.configuredClient(&resp.Diagnostics)
	if !ok {
		return
	}

	if err := client.DeleteAgentConfig(ctx, state.Purpose.ValueString()); err != nil && !errors.Is(err, traderapi.ErrAgentConfigNotFound) {
		resp.Diagnostics.AddError("Error deleting agent config", err.Error())
	}
}

func (r *agentConfigResource) ImportState(ctx context.Context, req resource.ImportStateRequest, resp *resource.ImportStateResponse) {
	resp.Diagnostics.Append(resp.State.SetAttribute(ctx, path.Root("purpose"), req.ID)...)
}

func (r *agentConfigResource) ValidateConfig(ctx context.Context, req resource.ValidateConfigRequest, resp *resource.ValidateConfigResponse) {
	var config agentConfigModel
	resp.Diagnostics.Append(req.Config.Get(ctx, &config)...)
	if resp.Diagnostics.HasError() || config.AgentGraph.IsNull() || config.AgentGraph.IsUnknown() {
		return
	}
	if err := validateAgentGraph(config.AgentGraph.ValueString()); err != nil {
		resp.Diagnostics.AddAttributeError(path.Root("agent_graph"), "Invalid agent_graph", err.Error())
	}
}

func (r *agentConfigResource) configuredClient(diagnostics *diag.Diagnostics) (*traderapi.Client, bool) {
	if r.client == nil {
		diagnostics.AddError("Provider Not Configured", "The t-rader provider must be configured before managing agent configs.")
		return nil, false
	}
	return r.client, true
}

func skillsFromAttribute(ctx context.Context, value types.Map, diagnostics *diag.Diagnostics) (map[string]string, bool) {
	var skills map[string]string
	diagnostics.Append(value.ElementsAs(ctx, &skills, false)...)
	if diagnostics.HasError() {
		return nil, false
	}
	return nonNilSkills(skills), true
}

func nonNilSkills(skills map[string]string) map[string]string {
	if skills == nil {
		return map[string]string{}
	}
	return skills
}

func skillsFromConfig(value any) (map[string]string, error) {
	skills, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("backend returned agent config skills with type %T", value)
	}
	result := make(map[string]string, len(skills))
	for name, content := range skills {
		text, ok := content.(string)
		if !ok {
			return nil, fmt.Errorf("backend returned agent config skill %q with type %T", name, content)
		}
		result[name] = text
	}
	return result, nil
}

type agentGraphPhase struct {
	key    string
	output map[string]any
}

// YAML 全体の schema は backend が所有するため、Terraform は参照関係の検証に必要な項目だけを読む。
func validateAgentGraph(content string) error {
	if strings.TrimSpace(content) == "" {
		return nil
	}

	var root map[string]any
	if err := yaml.Unmarshal([]byte(content), &root); err != nil {
		return fmt.Errorf("agent_graph is not valid YAML: %w", err)
	}
	phasesValue, ok := root["phases"].([]any)
	if !ok {
		return errors.New("agent_graph must contain a phases list")
	}

	seen := make(map[string]struct{}, len(phasesValue))
	earlier := make(map[string]agentGraphPhase, len(phasesValue))
	for _, phaseValue := range phasesValue {
		phaseMap, ok := phaseValue.(map[string]any)
		if !ok {
			return errors.New("each agent_graph phase must be a mapping")
		}
		key, ok := phaseMap["key"].(string)
		if !ok {
			return errors.New("each agent_graph phase must have a string key")
		}
		if _, exists := seen[key]; exists {
			return fmt.Errorf("phase key %q is duplicated", key)
		}
		seen[key] = struct{}{}

		output, ok := phaseMap["output"].(map[string]any)
		if !ok {
			output = map[string]any{}
		}
		if forEachValue, exists := phaseMap["for_each"]; exists && forEachValue != nil {
			forEach, ok := forEachValue.(string)
			if !ok {
				return fmt.Errorf("phase %q: for_each must be a string", key)
			}
			if err := validateAgentGraphForEach(key, forEach, earlier); err != nil {
				return err
			}
		}
		earlier[key] = agentGraphPhase{key: key, output: output}
	}
	return nil
}

func validateAgentGraphForEach(phase, forEach string, earlier map[string]agentGraphPhase) error {
	referencedKey, field, found := strings.Cut(forEach, ".")
	if !found {
		return fmt.Errorf("phase %q: for_each must be in the form \"<phase_key>.<field>\", got %q", phase, forEach)
	}
	referenced, found := earlier[referencedKey]
	if !found {
		return fmt.Errorf("phase %q: for_each references unknown or later phase %q", phase, referencedKey)
	}
	fieldValue, found := referenced.output[field]
	if !found {
		return fmt.Errorf("phase %q: for_each references field %q which is not defined in phase %q's output", phase, field, referenced.key)
	}
	fieldSchema, ok := fieldValue.(map[string]any)
	if !ok || fieldSchema["type"] != "array" {
		return fmt.Errorf("phase %q: for_each references field %q in phase %q, which is not an array", phase, field, referenced.key)
	}
	return nil
}
