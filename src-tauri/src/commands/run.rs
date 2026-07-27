use crate::*;

#[tauri::command]
pub(crate) async fn start_room_run(
    app: AppHandle,
    database: State<'_, Database>,
    runtime: State<'_, RuntimeState>,
    request: StartRunRequest,
) -> Result<StartRunResult, String> {
    let run_id = request.run_id.clone();
    let project_id = request.project_id.clone();
    {
        let mut active_runs = runtime.active_ship_runs.lock().await;
        if !active_runs.is_empty() {
            return Err("A Ship run is already active. Stop, wait for, or abandon it before starting another.".to_owned());
        }
        active_runs.insert(run_id.clone());
    }
    let result = execute_room_run(app.clone(), database.inner(), runtime.inner(), request).await;
    runtime.active_ship_runs.lock().await.remove(&run_id);
    if let Err(error) = result.as_ref() {
        let _ = finalize_unhandled_run_error(&app, database.inner(), &project_id, &run_id, error);
        runtime.cancellations.lock().await.remove(&run_id);
    }
    result
}
