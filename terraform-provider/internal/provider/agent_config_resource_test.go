package provider

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"reflect"
	"testing"

	"github.com/hashicorp/terraform-plugin-framework/diag"
	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/tfsdk"
	"github.com/hashicorp/terraform-plugin-framework/types"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
)

const testAgentConfigPurpose = "synthetic-purpose"

type agentConfigRequest struct {
	Method string
	Path   string
	Body   string
}

type agentConfigDiagnostic struct {
	Severity diag.Severity
	Summary  string
	Detail   string
}

func TestAgentConfigResourceCreateWritesAllSettings(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	var requests []agentConfigRequest
	client := newAgentConfigResourceClient(t, func(w http.ResponseWriter, r *http.Request, body []byte) {
		requests = append(requests, agentConfigRequest{Method: r.Method, Path: r.URL.Path, Body: string(body)})
		switch r.Method {
		case http.MethodPost:
			writeAgentConfigJSON(t, w, http.StatusCreated, map[string]any{
				"id":          "00000000-0000-4000-8000-000000000002",
				"purpose":     testAgentConfigPurpose,
				"agents_md":   "",
				"skills":      map[string]string{},
				"agent_graph": "",
				"created_at":  "2026-01-01T00:00:00Z",
				"updated_at":  "2026-01-01T00:00:00Z",
			})
		case http.MethodPut:
			w.WriteHeader(http.StatusOK)
		default:
			t.Errorf("unexpected API method %q", r.Method)
			w.WriteHeader(http.StatusMethodNotAllowed)
		}
	})
	schemaResponse := agentConfigResourceSchema(t)
	plan := newAgentConfigPlan(t, schemaResponse.Schema, agentConfigModel{
		Purpose:    types.StringValue(testAgentConfigPurpose),
		AgentsMd:   types.StringValue("synthetic instructions"),
		Skills:     newAgentConfigSkills(t, map[string]string{"sample": "synthetic skill contents"}),
		AgentGraph: types.StringValue(""),
	})
	response := resource.CreateResponse{State: tfsdk.State{Schema: schemaResponse.Schema}}

	(&agentConfigResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)
	var state agentConfigModel
	response.Diagnostics.Append(response.State.Get(ctx, &state)...)

	got := struct {
		Requests  []agentConfigRequest
		State     agentConfigModel
		HasErrors bool
	}{requests, state, response.Diagnostics.HasError()}
	want := struct {
		Requests  []agentConfigRequest
		State     agentConfigModel
		HasErrors bool
	}{
		Requests: []agentConfigRequest{
			{Method: http.MethodPost, Path: "/api/agent-configs", Body: `{"purpose":"synthetic-purpose"}`},
			{Method: http.MethodPut, Path: "/api/agent-configs/synthetic-purpose/skills", Body: `{"skills":{"sample":"synthetic skill contents"}}`},
			{Method: http.MethodPut, Path: "/api/agent-configs/synthetic-purpose/agents-md", Body: `{"content":"synthetic instructions"}`},
			{Method: http.MethodPut, Path: "/api/agent-configs/synthetic-purpose/agent-graph", Body: `{"content":""}`},
		},
		State: agentConfigModel{
			Purpose:    types.StringValue(testAgentConfigPurpose),
			AgentsMd:   types.StringValue("synthetic instructions"),
			Skills:     newAgentConfigSkills(t, map[string]string{"sample": "synthetic skill contents"}),
			AgentGraph: types.StringValue(""),
		},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("create output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestAgentConfigResourceReadLoadsAllSettings(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	var requests []agentConfigRequest
	client := newAgentConfigResourceClient(t, func(w http.ResponseWriter, r *http.Request, body []byte) {
		requests = append(requests, agentConfigRequest{Method: r.Method, Path: r.URL.Path, Body: string(body)})
		writeAgentConfigJSON(t, w, http.StatusOK, map[string]any{
			"agents_md":   "loaded instructions",
			"skills":      map[string]string{"sample": "loaded skill contents"},
			"agent_graph": "phases: []",
		})
	})
	schemaResponse := agentConfigResourceSchema(t)
	state := newAgentConfigState(t, schemaResponse.Schema, agentConfigModel{
		Purpose:    types.StringValue(testAgentConfigPurpose),
		AgentsMd:   types.StringValue("old instructions"),
		Skills:     newAgentConfigSkills(t, map[string]string{}),
		AgentGraph: types.StringValue(""),
	})
	response := resource.ReadResponse{State: tfsdk.State{Raw: state.Raw, Schema: schemaResponse.Schema}}

	(&agentConfigResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)
	var result agentConfigModel
	response.Diagnostics.Append(response.State.Get(ctx, &result)...)

	got := struct {
		Requests  []agentConfigRequest
		State     agentConfigModel
		HasErrors bool
	}{requests, result, response.Diagnostics.HasError()}
	want := struct {
		Requests  []agentConfigRequest
		State     agentConfigModel
		HasErrors bool
	}{
		Requests: []agentConfigRequest{{Method: http.MethodGet, Path: "/api/agent-configs/synthetic-purpose/agent-config"}},
		State: agentConfigModel{
			Purpose:    types.StringValue(testAgentConfigPurpose),
			AgentsMd:   types.StringValue("loaded instructions"),
			Skills:     newAgentConfigSkills(t, map[string]string{"sample": "loaded skill contents"}),
			AgentGraph: types.StringValue("phases: []"),
		},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("read output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestAgentConfigResourceUpdateReplacesSettings(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	var requests []agentConfigRequest
	client := newAgentConfigResourceClient(t, func(w http.ResponseWriter, r *http.Request, body []byte) {
		requests = append(requests, agentConfigRequest{Method: r.Method, Path: r.URL.Path, Body: string(body)})
		w.WriteHeader(http.StatusOK)
	})
	schemaResponse := agentConfigResourceSchema(t)
	state := newAgentConfigState(t, schemaResponse.Schema, agentConfigModel{
		Purpose:    types.StringValue(testAgentConfigPurpose),
		AgentsMd:   types.StringValue("old instructions"),
		Skills:     newAgentConfigSkills(t, map[string]string{"old": "old skill"}),
		AgentGraph: types.StringValue("phases: []"),
	})
	plan := newAgentConfigPlan(t, schemaResponse.Schema, agentConfigModel{
		Purpose:    types.StringValue(testAgentConfigPurpose),
		AgentsMd:   types.StringValue("updated instructions"),
		Skills:     newAgentConfigSkills(t, map[string]string{"sample": "updated skill"}),
		AgentGraph: types.StringValue("phases: []"),
	})
	response := resource.UpdateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: schemaResponse.Schema}}

	(&agentConfigResource{client: client}).Update(ctx, resource.UpdateRequest{Plan: plan, State: state}, &response)
	var result agentConfigModel
	response.Diagnostics.Append(response.State.Get(ctx, &result)...)

	got := struct {
		Requests  []agentConfigRequest
		State     agentConfigModel
		HasErrors bool
	}{requests, result, response.Diagnostics.HasError()}
	want := struct {
		Requests  []agentConfigRequest
		State     agentConfigModel
		HasErrors bool
	}{
		Requests: []agentConfigRequest{
			{Method: http.MethodPut, Path: "/api/agent-configs/synthetic-purpose/skills", Body: `{"skills":{"sample":"updated skill"}}`},
			{Method: http.MethodPut, Path: "/api/agent-configs/synthetic-purpose/agents-md", Body: `{"content":"updated instructions"}`},
			{Method: http.MethodPut, Path: "/api/agent-configs/synthetic-purpose/agent-graph", Body: `{"content":"phases: []"}`},
		},
		State: agentConfigModel{
			Purpose:    types.StringValue(testAgentConfigPurpose),
			AgentsMd:   types.StringValue("updated instructions"),
			Skills:     newAgentConfigSkills(t, map[string]string{"sample": "updated skill"}),
			AgentGraph: types.StringValue("phases: []"),
		},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("update output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestAgentConfigResourceUpdateStopsAfterFailedPut(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	var requests []agentConfigRequest
	client := newAgentConfigResourceClient(t, func(w http.ResponseWriter, r *http.Request, body []byte) {
		requests = append(requests, agentConfigRequest{Method: r.Method, Path: r.URL.Path, Body: string(body)})
		if r.URL.Path == "/api/agent-configs/synthetic-purpose/agents-md" {
			http.Error(w, "synthetic update failure", http.StatusBadRequest)
			return
		}
		w.WriteHeader(http.StatusOK)
	})
	schemaResponse := agentConfigResourceSchema(t)
	state := newAgentConfigState(t, schemaResponse.Schema, agentConfigModel{
		Purpose:    types.StringValue(testAgentConfigPurpose),
		AgentsMd:   types.StringValue("old instructions"),
		Skills:     newAgentConfigSkills(t, map[string]string{}),
		AgentGraph: types.StringValue("phases: []"),
	})
	plan := newAgentConfigPlan(t, schemaResponse.Schema, agentConfigModel{
		Purpose:    types.StringValue(testAgentConfigPurpose),
		AgentsMd:   types.StringValue("updated instructions"),
		Skills:     newAgentConfigSkills(t, map[string]string{"sample": "updated skill"}),
		AgentGraph: types.StringValue("phases: []"),
	})
	response := resource.UpdateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: schemaResponse.Schema}}

	(&agentConfigResource{client: client}).Update(ctx, resource.UpdateRequest{Plan: plan, State: state}, &response)

	got := struct {
		Requests    []agentConfigRequest
		Diagnostics []agentConfigDiagnostic
	}{requests, agentConfigDiagnostics(response.Diagnostics)}
	want := struct {
		Requests    []agentConfigRequest
		Diagnostics []agentConfigDiagnostic
	}{
		Requests: []agentConfigRequest{
			{Method: http.MethodPut, Path: "/api/agent-configs/synthetic-purpose/skills", Body: `{"skills":{"sample":"updated skill"}}`},
			{Method: http.MethodPut, Path: "/api/agent-configs/synthetic-purpose/agents-md", Body: `{"content":"updated instructions"}`},
		},
		Diagnostics: []agentConfigDiagnostic{{Severity: diag.SeverityError, Summary: "Error updating agent config", Detail: "backend returned HTTP 400: synthetic update failure"}},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("failed update output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestAgentConfigResourceCreateRetainsPurposeWhenRollbackFails(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	var requests []agentConfigRequest
	client := newAgentConfigResourceClient(t, func(w http.ResponseWriter, r *http.Request, body []byte) {
		requests = append(requests, agentConfigRequest{Method: r.Method, Path: r.URL.Path, Body: string(body)})
		switch r.Method {
		case http.MethodPost:
			writeAgentConfigJSON(t, w, http.StatusCreated, map[string]any{
				"id":          "00000000-0000-4000-8000-000000000002",
				"purpose":     testAgentConfigPurpose,
				"agents_md":   "",
				"skills":      map[string]string{},
				"agent_graph": "",
				"created_at":  "2026-01-01T00:00:00Z",
				"updated_at":  "2026-01-01T00:00:00Z",
			})
		case http.MethodPut:
			if r.URL.Path == "/api/agent-configs/synthetic-purpose/agents-md" {
				http.Error(w, "synthetic update failure", http.StatusBadRequest)
				return
			}
			w.WriteHeader(http.StatusOK)
		case http.MethodDelete:
			http.Error(w, "synthetic rollback failure", http.StatusInternalServerError)
		default:
			t.Errorf("unexpected API method %q", r.Method)
			w.WriteHeader(http.StatusMethodNotAllowed)
		}
	})
	schemaResponse := agentConfigResourceSchema(t)
	plan := newAgentConfigPlan(t, schemaResponse.Schema, agentConfigModel{
		Purpose:    types.StringValue(testAgentConfigPurpose),
		AgentsMd:   types.StringValue("synthetic instructions"),
		Skills:     newAgentConfigSkills(t, map[string]string{"sample": "synthetic skill contents"}),
		AgentGraph: types.StringValue("phases: []"),
	})
	response := resource.CreateResponse{State: tfsdk.State{Schema: schemaResponse.Schema}}

	(&agentConfigResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)
	var state agentConfigModel
	response.Diagnostics.Append(response.State.Get(ctx, &state)...)

	got := struct {
		Requests    []agentConfigRequest
		State       agentConfigModel
		Diagnostics []agentConfigDiagnostic
	}{requests, state, agentConfigDiagnostics(response.Diagnostics)}
	want := struct {
		Requests    []agentConfigRequest
		State       agentConfigModel
		Diagnostics []agentConfigDiagnostic
	}{
		Requests: []agentConfigRequest{
			{Method: http.MethodPost, Path: "/api/agent-configs", Body: `{"purpose":"synthetic-purpose"}`},
			{Method: http.MethodPut, Path: "/api/agent-configs/synthetic-purpose/skills", Body: `{"skills":{"sample":"synthetic skill contents"}}`},
			{Method: http.MethodPut, Path: "/api/agent-configs/synthetic-purpose/agents-md", Body: `{"content":"synthetic instructions"}`},
			{Method: http.MethodDelete, Path: "/api/agent-configs/synthetic-purpose"},
		},
		State: agentConfigModel{
			Purpose:    types.StringValue(testAgentConfigPurpose),
			AgentsMd:   types.StringNull(),
			Skills:     types.MapNull(types.StringType),
			AgentGraph: types.StringNull(),
		},
		Diagnostics: []agentConfigDiagnostic{{
			Severity: diag.SeverityError,
			Summary:  "Error configuring created agent config",
			Detail:   "backend returned HTTP 400: synthetic update failure; remove partially created agent config: backend returned HTTP 500: synthetic rollback failure",
		}},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("failed create output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestAgentConfigResourceDeleteRemovesConfig(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	var requests []agentConfigRequest
	client := newAgentConfigResourceClient(t, func(w http.ResponseWriter, r *http.Request, body []byte) {
		requests = append(requests, agentConfigRequest{Method: r.Method, Path: r.URL.Path, Body: string(body)})
		w.WriteHeader(http.StatusNoContent)
	})
	schemaResponse := agentConfigResourceSchema(t)
	state := newAgentConfigState(t, schemaResponse.Schema, agentConfigModel{
		Purpose:    types.StringValue(testAgentConfigPurpose),
		AgentsMd:   types.StringValue("synthetic instructions"),
		Skills:     newAgentConfigSkills(t, map[string]string{}),
		AgentGraph: types.StringValue(""),
	})
	response := &resource.DeleteResponse{}

	(&agentConfigResource{client: client}).Delete(ctx, resource.DeleteRequest{State: state}, response)

	got := struct {
		Requests  []agentConfigRequest
		HasErrors bool
	}{requests, response.Diagnostics.HasError()}
	want := struct {
		Requests  []agentConfigRequest
		HasErrors bool
	}{Requests: []agentConfigRequest{{Method: http.MethodDelete, Path: "/api/agent-configs/synthetic-purpose"}}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("delete output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestAgentConfigResourceImportUsesPurpose(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	schemaResponse := agentConfigResourceSchema(t)
	state := newAgentConfigState(t, schemaResponse.Schema, agentConfigModel{
		Purpose:    types.StringNull(),
		AgentsMd:   types.StringNull(),
		Skills:     types.MapNull(types.StringType),
		AgentGraph: types.StringNull(),
	})
	response := resource.ImportStateResponse{State: state}
	(&agentConfigResource{}).ImportState(ctx, resource.ImportStateRequest{ID: testAgentConfigPurpose}, &response)
	var purpose types.String
	response.Diagnostics.Append(response.State.GetAttribute(ctx, path.Root("purpose"), &purpose)...)

	got := struct {
		Purpose   types.String
		HasErrors bool
	}{purpose, response.Diagnostics.HasError()}
	want := struct {
		Purpose   types.String
		HasErrors bool
	}{types.StringValue(testAgentConfigPurpose), false}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("import output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestAgentConfigResourceValidateConfigChecksAgentGraph(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name        string
		graph       string
		diagnostics []agentConfigDiagnostic
	}{
		{
			name:  "empty graph is accepted",
			graph: " ",
		},
		{
			name: "earlier array output can be referenced",
			graph: `phases:
  - key: sample
    output:
      items:
        type: array
  - key: followup
    for_each: sample.items
`,
		},
		{
			name:  "invalid YAML is rejected",
			graph: "phases: [",
			diagnostics: []agentConfigDiagnostic{{
				Severity: diag.SeverityError,
				Summary:  "Invalid agent_graph",
				Detail:   "agent_graph is not valid YAML: yaml: line 1: did not find expected node content",
			}},
		},
		{
			name: "duplicate phase key is rejected",
			graph: `phases:
  - key: sample
  - key: sample
`,
			diagnostics: []agentConfigDiagnostic{{
				Severity: diag.SeverityError,
				Summary:  "Invalid agent_graph",
				Detail:   `phase key "sample" is duplicated`,
			}},
		},
		{
			name: "for each format is rejected",
			graph: `phases:
  - key: sample
  - key: followup
    for_each: sample
`,
			diagnostics: []agentConfigDiagnostic{{
				Severity: diag.SeverityError,
				Summary:  "Invalid agent_graph",
				Detail:   `phase "followup": for_each must be in the form "<phase_key>.<field>", got "sample"`,
			}},
		},
		{
			name: "for each must reference an earlier phase",
			graph: `phases:
  - key: followup
    for_each: later.items
  - key: later
    output:
      items:
        type: array
`,
			diagnostics: []agentConfigDiagnostic{{
				Severity: diag.SeverityError,
				Summary:  "Invalid agent_graph",
				Detail:   `phase "followup": for_each references unknown or later phase "later"`,
			}},
		},
		{
			name: "for each must reference an output field",
			graph: `phases:
  - key: sample
    output: {}
  - key: followup
    for_each: sample.items
`,
			diagnostics: []agentConfigDiagnostic{{
				Severity: diag.SeverityError,
				Summary:  "Invalid agent_graph",
				Detail:   `phase "followup": for_each references field "items" which is not defined in phase "sample"'s output`,
			}},
		},
		{
			name: "for each output must be an array",
			graph: `phases:
  - key: sample
    output:
      items:
        type: string
  - key: followup
    for_each: sample.items
`,
			diagnostics: []agentConfigDiagnostic{{
				Severity: diag.SeverityError,
				Summary:  "Invalid agent_graph",
				Detail:   `phase "followup": for_each references field "items" in phase "sample", which is not an array`,
			}},
		},
	}

	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			schemaResponse := agentConfigResourceSchema(t)
			plan := newAgentConfigPlan(t, schemaResponse.Schema, agentConfigModel{
				Purpose:    types.StringValue(testAgentConfigPurpose),
				AgentsMd:   types.StringValue("synthetic instructions"),
				Skills:     newAgentConfigSkills(t, map[string]string{}),
				AgentGraph: types.StringValue(testCase.graph),
			})
			response := &resource.ValidateConfigResponse{}
			(&agentConfigResource{}).ValidateConfig(context.Background(), resource.ValidateConfigRequest{
				Config: tfsdk.Config{Raw: plan.Raw, Schema: plan.Schema},
			}, response)

			got := agentConfigDiagnostics(response.Diagnostics)
			if !reflect.DeepEqual(got, testCase.diagnostics) {
				t.Fatalf("agent_graph validation mismatch: got=%#v want=%#v", got, testCase.diagnostics)
			}
		})
	}
}

func agentConfigResourceSchema(t *testing.T) resource.SchemaResponse {
	t.Helper()
	var response resource.SchemaResponse
	(&agentConfigResource{}).Schema(context.Background(), resource.SchemaRequest{}, &response)
	return response
}

func newAgentConfigPlan(t *testing.T, resourceSchema schema.Schema, model agentConfigModel) tfsdk.Plan {
	t.Helper()
	plan := tfsdk.Plan{Schema: resourceSchema}
	if diagnostics := plan.Set(context.Background(), model); diagnostics.HasError() {
		t.Fatalf("build agent config plan: %v", diagnostics)
	}
	return plan
}

func newAgentConfigState(t *testing.T, resourceSchema schema.Schema, model agentConfigModel) tfsdk.State {
	t.Helper()
	state := tfsdk.State{Schema: resourceSchema}
	if diagnostics := state.Set(context.Background(), model); diagnostics.HasError() {
		t.Fatalf("build agent config state: %v", diagnostics)
	}
	return state
}

func newAgentConfigSkills(t *testing.T, values map[string]string) types.Map {
	t.Helper()
	skills, diagnostics := types.MapValueFrom(context.Background(), types.StringType, values)
	if diagnostics.HasError() {
		t.Fatalf("build agent config skills: %v", diagnostics)
	}
	return skills
}

func newAgentConfigResourceClient(t *testing.T, handler func(http.ResponseWriter, *http.Request, []byte)) *traderapi.Client {
	t.Helper()
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		body, err := io.ReadAll(r.Body)
		if err != nil {
			t.Errorf("read API request body: %v", err)
			return
		}
		handler(w, r, body)
	}))
	t.Cleanup(server.Close)
	client, err := traderapi.New(server.URL, "", "")
	if err != nil {
		t.Fatalf("create test API client: %v", err)
	}
	return client
}

func writeAgentConfigJSON(t *testing.T, w http.ResponseWriter, status int, value any) {
	t.Helper()
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	if err := json.NewEncoder(w).Encode(value); err != nil {
		t.Errorf("encode API response: %v", err)
	}
}

func agentConfigDiagnostics(diagnostics diag.Diagnostics) []agentConfigDiagnostic {
	var result []agentConfigDiagnostic
	for _, diagnostic := range diagnostics {
		result = append(result, agentConfigDiagnostic{
			Severity: diagnostic.Severity(),
			Summary:  diagnostic.Summary(),
			Detail:   diagnostic.Detail(),
		})
	}
	return result
}
