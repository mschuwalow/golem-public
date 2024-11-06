// Copyright 2024 Golem Cloud
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::command::ComponentRefSplit;
use crate::model::application_manifest::load_app;
use crate::model::{
    ComponentName, Format, GolemError, GolemResult, PathBufOrStdin, WorkerUpdateMode,
};
use crate::service::component::ComponentService;
use crate::service::deploy::DeployService;
use crate::service::project::ProjectResolver;
use clap::Subcommand;
use golem_client::model::ComponentType;
use golem_wasm_rpc_stubgen::commands::declarative::ApplicationResolveMode;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Subcommand, Debug)]
#[command()]
pub enum ComponentSubCommand<ProjectRef: clap::Args, ComponentRef: clap::Args> {
    /// Creates a new component by uploading the component WASM.
    ///
    /// If neither `component-file` nor `app` is specified, the command will look for the manifest in the current directory and all parent directories.
    /// It will then look for the component in the manifest and use the settings there.
    #[command(alias = "create", verbatim_doc_comment)]
    Add {
        /// The WASM file to be used as a Golem component
        ///
        /// Conflics with `app` flag.
        #[arg(value_name = "component-file", value_hint = clap::ValueHint::FilePath)]
        component_file: Option<PathBufOrStdin>, // TODO: validate exists

        /// The newly created component's owner project
        #[command(flatten)]
        project_ref: ProjectRef,

        /// Name of the newly created component
        ///
        /// If 'component-file' is specified, this flag controls the name of the component.
        /// If 'component-file' is not specified, or 'app' is specified, this flag is used to resolve the component from the app manifest.
        #[arg(short, long, verbatim_doc_comment)]
        component_name: ComponentName,

        /// The component type. If none specified, the command creates a Durable component.
        ///
        /// Conflicts with `app` flag.
        #[command(flatten, verbatim_doc_comment)]
        component_type: ComponentTypeArg,

        /// Application manifest to use. Can be specified multiple times.
        /// The component-name flag is used to resolve the component from the app manifest.
        /// Other settings are then taken from the app manifest.
        ///
        /// Conflicts with `component_file` flag.
        /// Conflicts with `ephemeral` flag.
        /// Conflicts with `durable` flag.
        #[arg(long, short, conflicts_with_all = vec!["component_file", "component-type-flag"], verbatim_doc_comment)]
        app: Vec<PathBuf>,

        /// Do not ask for confirmation for performing an update in case the component already exists
        #[arg(short = 'y', long)]
        non_interactive: bool,
    },
    /// Updates an existing component by uploading a new version of its WASM
    ///
    /// If neither `component-file` nor `app` is specified, the command will look for the manifest in the current directory and all parent directories.
    /// It will then look for the component in the manifest and use the settings there.
    #[command()]
    Update {
        /// The WASM file to be used as a new version of the Golem component
        ///
        /// Conflics with `app` flag.
        #[arg(value_name = "component-file", value_hint = clap::ValueHint::FilePath, verbatim_doc_comment)]
        component_file: Option<PathBufOrStdin>, // TODO: validate exists

        /// The component to update
        #[command(flatten)]
        component_name_or_uri: ComponentRef,

        /// The updated component's type. If none specified, the previous version's type is used.
        ///
        /// Conflicts with `app` flag.
        #[command(flatten, verbatim_doc_comment)]
        component_type: UpdatedComponentTypeArg,

        /// Application manifest to use. Can be specified multiple times.
        /// The component-name flag is used to resolve the component from the app manifest.
        /// Other settings are then taken from the app manifest.
        ///
        /// Conflicts with `component-file` flag.
        /// Conflicts with `ephemeral` flag.
        /// Conflicts with `durable` flag.
        #[arg(long, short)]
        app: Vec<PathBuf>,

        /// Try to automatically update all existing workers to the new version
        #[arg(long, default_value_t = false)]
        try_update_workers: bool,

        /// Update mode - auto or manual
        #[arg(long, default_value = "auto", requires = "try_update_workers")]
        update_mode: WorkerUpdateMode,

        /// Do not ask for confirmation for creating a new component in case it does not exist
        #[arg(short = 'y', long)]
        non_interactive: bool,
    },
    /// Lists the existing components
    #[command()]
    List {
        /// The project to list components from
        #[command(flatten)]
        project_ref: ProjectRef,

        /// Optionally look for only components matching a given name
        #[arg(short, long)]
        component_name: Option<ComponentName>,
    },
    /// Get component
    #[command()]
    Get {
        /// The Golem component
        #[command(flatten)]
        component_name_or_uri: ComponentRef,

        /// The version of the component
        #[arg(short = 't', long)]
        version: Option<u64>,
    },
    /// Try to automatically update all existing workers to the latest version
    #[command()]
    TryUpdateWorkers {
        /// The component to redeploy
        #[command(flatten)]
        component_name_or_uri: ComponentRef,

        /// Update mode - auto or manual
        #[arg(long, default_value = "auto")]
        update_mode: WorkerUpdateMode,
    },
    /// Redeploy all workers of a component using the latest version
    #[command()]
    Redeploy {
        /// The component to redeploy
        #[command(flatten)]
        component_name_or_uri: ComponentRef,

        /// Do not ask for confirmation
        #[arg(short = 'y', long)]
        non_interactive: bool,
    },
}

#[derive(clap::Args, Debug, Clone)]
#[group(required = false, multiple = false)]
pub struct ComponentTypeArg {
    /// Create an Ephemeral component. If not specified, the command creates a Durable component.
    #[arg(long, group = "component-type-flag")]
    ephemeral: bool,

    /// Create a Durable component. This is the default.
    #[arg(long, group = "component-type-flag")]
    durable: bool,
}

impl ComponentTypeArg {
    pub fn component_type(&self) -> ComponentType {
        if self.ephemeral {
            ComponentType::Ephemeral
        } else {
            ComponentType::Durable
        }
    }
}

#[derive(clap::Args, Debug, Clone)]
#[group(required = false, multiple = false)]
pub struct UpdatedComponentTypeArg {
    /// Create an Ephemeral component. If not specified, the previous version's type will be used.
    #[arg(long, group = "component-type-flag")]
    ephemeral: bool,

    /// Create a Durable component. If not specified, the previous version's type will be used.
    #[arg(long, group = "component-type-flag")]
    durable: bool,
}

impl UpdatedComponentTypeArg {
    pub fn optional_component_type(&self) -> Option<ComponentType> {
        if self.ephemeral {
            Some(ComponentType::Ephemeral)
        } else if self.durable {
            Some(ComponentType::Durable)
        } else {
            None
        }
    }
}

impl<
        ProjectRef: clap::Args + Send + Sync + 'static,
        ComponentRef: ComponentRefSplit<ProjectRef> + clap::Args,
    > ComponentSubCommand<ProjectRef, ComponentRef>
{
    pub async fn handle<ProjectContext: Clone + Send + Sync>(
        self,
        format: Format,
        service: Arc<dyn ComponentService<ProjectContext = ProjectContext> + Send + Sync>,
        deploy_service: Arc<dyn DeployService<ProjectContext = ProjectContext> + Send + Sync>,
        projects: &(dyn ProjectResolver<ProjectRef, ProjectContext> + Send + Sync),
    ) -> Result<GolemResult, GolemError> {
        match self {
            ComponentSubCommand::Add {
                project_ref,
                component_name,
                component_file: Some(component_file),
                component_type,
                app: _,
                non_interactive,
            } => {
                let project_id = projects.resolve_id_or_default(project_ref).await?;

                service
                    .add(
                        component_name,
                        component_file,
                        component_type.component_type(),
                        Some(project_id),
                        non_interactive,
                        format,
                        vec![],
                    )
                    .await
            }
            ComponentSubCommand::Add {
                project_ref,
                component_name,
                component_file: None,
                component_type: _,
                app,
                non_interactive,
            } => {
                let project_id = projects.resolve_id_or_default(project_ref).await?;

                let app_resolve_mode = if app.is_empty() {
                    ApplicationResolveMode::Automatic
                } else {
                    ApplicationResolveMode::Explicit(app)
                };

                let app = load_app(&app_resolve_mode)?;

                let component =
                    if let Some(component) = app.wasm_components_by_name.get(&component_name.0) {
                        component
                    } else {
                        return Err(GolemError(format!(
                            "Component {} not found in the app manifest",
                            component_name
                        )));
                    };

                let component_file =
                    PathBufOrStdin::Path(app.component_output_wasm(&component_name.0));

                service
                    .add(
                        component_name,
                        component_file,
                        component.component_type,
                        Some(project_id),
                        non_interactive,
                        format,
                        component.files.clone(),
                    )
                    .await
            }
            ComponentSubCommand::Update {
                component_name_or_uri,
                component_file: Some(component_file),
                component_type,
                app: _,
                try_update_workers,
                update_mode,
                non_interactive,
            } => {
                let (component_name_or_uri, project_ref) = component_name_or_uri.split();
                let project_id = projects.resolve_id_or_default_opt(project_ref).await?;
                let mut result = service
                    .update(
                        component_name_or_uri.clone(),
                        component_file,
                        component_type.optional_component_type(),
                        project_id.clone(),
                        non_interactive,
                        format,
                        vec![],
                    )
                    .await?;

                if try_update_workers {
                    let deploy_result = deploy_service
                        .try_update_all_workers(component_name_or_uri, project_id, update_mode)
                        .await?;
                    result = result.merge(deploy_result);
                }
                Ok(result)
            }
            ComponentSubCommand::Update {
                component_name_or_uri,
                non_interactive,
                component_file: None,
                component_type: _,
                app,
                try_update_workers,
                update_mode,
            } => {
                let (component_name_or_uri, project_ref) = component_name_or_uri.split();

                let component_name = service
                    .resolve_component_name(&component_name_or_uri)
                    .await?;

                let project_id = projects.resolve_id_or_default_opt(project_ref).await?;

                let app_resolve_mode = if app.is_empty() {
                    ApplicationResolveMode::Automatic
                } else {
                    ApplicationResolveMode::Explicit(app)
                };

                let app = load_app(&app_resolve_mode)?;

                let component =
                    if let Some(component) = app.wasm_components_by_name.get(&component_name) {
                        component
                    } else {
                        return Err(GolemError(format!(
                            "Component {} not found in the app manifest",
                            component_name
                        )));
                    };

                let component_file =
                    PathBufOrStdin::Path(app.component_output_wasm(&component_name));

                let mut result = service
                    .update(
                        component_name_or_uri.clone(),
                        component_file,
                        Some(component.component_type),
                        project_id.clone(),
                        non_interactive,
                        format,
                        component.files.clone(),
                    )
                    .await?;

                if try_update_workers {
                    let deploy_result = deploy_service
                        .try_update_all_workers(component_name_or_uri, project_id, update_mode)
                        .await?;
                    result = result.merge(deploy_result);
                }
                Ok(result)
            }
            ComponentSubCommand::List {
                project_ref,
                component_name,
            } => {
                let project_id = projects.resolve_id_or_default(project_ref).await?;
                service.list(component_name, Some(project_id)).await
            }
            ComponentSubCommand::Get {
                component_name_or_uri,
                version,
            } => {
                let (component_name_or_uri, project_ref) = component_name_or_uri.split();
                let project_id = projects.resolve_id_or_default_opt(project_ref).await?;
                service
                    .get(component_name_or_uri, version, project_id)
                    .await
            }
            ComponentSubCommand::TryUpdateWorkers {
                component_name_or_uri,
                update_mode,
            } => {
                let (component_name_or_uri, project_ref) = component_name_or_uri.split();
                let project_id = projects.resolve_id_or_default_opt(project_ref).await?;
                deploy_service
                    .try_update_all_workers(component_name_or_uri, project_id, update_mode)
                    .await
            }
            ComponentSubCommand::Redeploy {
                component_name_or_uri,
                non_interactive,
            } => {
                let (component_name_or_uri, project_ref) = component_name_or_uri.split();
                let project_id = projects.resolve_id_or_default_opt(project_ref).await?;
                deploy_service
                    .redeploy(component_name_or_uri, project_id, non_interactive, format)
                    .await
            }
        }
    }
}
