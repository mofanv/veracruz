//! Veracruz-server-specific tests
//!
//! One of the main integration tests for Veracruz, as a lot of material is
//! imported directly or indirectly via these tests.
//!
//! ## Authors
//!
//! The Veracruz Development Team.
//!
//! ## Licensing and copyright notice
//!
//! See the `LICENSE_MIT.markdown` file in the Veracruz root directory for
//! information on licensing and copyright.

mod tests {
    use actix_rt::System;
    use env_logger;
    use lazy_static::lazy_static;
    use log::{debug, info, LevelFilter};
    use ring;

    use serde::Deserialize;
    use transport_protocol;
    use veracruz_server::veracruz_server::*;
    #[cfg(feature = "nitro")]
    use veracruz_server::VeracruzServerNitro as VeracruzServerEnclave;
    #[cfg(feature = "sgx")]
    use veracruz_server::VeracruzServerSGX as VeracruzServerEnclave;
    #[cfg(feature = "tz")]
    use veracruz_server::VeracruzServerTZ as VeracruzServerEnclave;
    use veracruz_utils::{platform::Platform, policy::policy::Policy, VERACRUZ_RUNTIME_HASH_EXTENSION_ID};
    use proxy_attestation_server;
    use std::{
        collections::HashMap,
        collections::HashSet,
        io::{Read, Write},
        path::Path,
        sync::{
            atomic::{AtomicBool, AtomicU32, Ordering},
            Mutex, Once,
        },
        thread,
        time::Instant,
        vec::Vec,
    };

    // Policy files
    const ONE_DATA_SOURCE_POLICY: &'static str = "../test-collateral/one_data_source_policy.json";
    const GET_RANDOM_POLICY: &'static str = "../test-collateral/get_random_policy.json";
    const LINEAR_REGRESSION_POLICY: &'static str = "../test-collateral/one_data_source_policy.json";
    const TWO_DATA_SOURCE_STRING_EDIT_DISTANCE_POLICY: &'static str =
        "../test-collateral/two_data_source_string_edit_distance_policy.json";
    const TWO_DATA_SOURCE_INTERSECTION_SET_POLICY: &'static str =
        "../test-collateral/two_data_source_intersection_set_policy.json";
    const TWO_DATA_SOURCE_PRIVATE_SET_INTERSECTION_POLICY: &'static str =
        "../test-collateral/two_data_source_private_set_intersection_policy.json";
    const MULTIPLE_KEY_POLICY: &'static str = "../test-collateral/test_multiple_key_policy.json";
    const IDASH2017_POLICY: &'static str =
        "../test-collateral/idash2017_logistic_regression_policy.json";
    const MACD_POLICY: &'static str =
        "../test-collateral/moving_average_convergence_divergence.json";
    const PRIVATE_SET_INTER_SUM_POLICY: &'static str =
        "../test-collateral/private_set_intersection_sum.json";
    const NUMBER_STREAM_ACCUMULATION_POLICY: &'static str =
        "../test-collateral/number-stream-accumulation.json";
    const BASIC_FILE_READ_WRITE_POLICY: &'static str =
        "../test-collateral/basic_file_read_write.json";
    const DEEP_LEARNING_SERVER_POLICY: &'static str =
	"../test-collateral/deep_learning_server.json";

    const CA_CERT: &'static str = "../test-collateral/CACert.pem";
    const CA_KEY: &'static str = "../test-collateral/CAKey.pem";
    const CLIENT_CERT: &'static str = "../test-collateral/client_rsa_cert.pem";
    const CLIENT_KEY: &'static str = "../test-collateral/client_rsa_key.pem";
    const UNAUTHORIZED_CERT: &'static str = "../test-collateral/data_client_cert.pem";
    const UNAUTHORIZED_KEY: &'static str = "../test-collateral/data_client_key.pem"; 
   // Programs
    const RANDOM_SOURCE_WASM: &'static str = "../test-collateral/random-source.wasm";
    const READ_FILE_WASM: &'static str = "../test-collateral/read-file.wasm";
    const LINEAR_REGRESSION_WASM: &'static str = "../test-collateral/linear-regression.wasm";
    const STRING_EDIT_DISTANCE_WASM: &'static str = "../test-collateral/string-edit-distance.wasm";
    const CUSTOMER_ADS_INTERSECTION_SET_SUM_WASM: &'static str =
        "../test-collateral/intersection-set-sum.wasm";
    const PERSON_SET_INTERSECTION_WASM: &'static str =
        "../test-collateral/private-set-intersection.wasm";
    const LOGISTICS_REGRESSION_WASM: &'static str =
        "../test-collateral/idash2017-logistic-regression.wasm";
    const MACD_WASM: &'static str = "../test-collateral/moving-average-convergence-divergence.wasm";
    const INTERSECTION_SET_SUM_WASM: &'static str =
        "../test-collateral/private-set-intersection-sum.wasm";
    const NUMBER_STREM_WASM: &'static str = "../test-collateral/number-stream-accumulation.wasm";
    const DEEP_LEARNING_WASM: &'static str = "../test-collateral/dl-server.wasm";
    // Data
    const LINEAR_REGRESSION_DATA: &'static str = "../test-collateral/linear-regression.dat";
    const INTERSECTION_SET_SUM_CUSTOMER_DATA: &'static str =
        "../test-collateral/intersection-customer.dat";
    const INTERSECTION_SET_SUM_ADVERTISEMENT_DATA: &'static str =
        "../test-collateral/intersection-advertisement-viewer.dat";
    const STRING_1_DATA: &'static str = "../test-collateral/hello-world-1.dat";
    const STRING_2_DATA: &'static str = "../test-collateral/hello-world-2.dat";
    const PERSON_SET_1_DATA: &'static str = "../test-collateral/private-set-1.dat";
    const PERSON_SET_2_DATA: &'static str = "../test-collateral/private-set-2.dat";
    const SINGLE_F64_DATA: &'static str = "../test-collateral/number-stream-init.dat";
    const VEC_F64_1_DATA: &'static str = "../test-collateral/number-stream-1.dat";
    const VEC_F64_2_DATA: &'static str = "../test-collateral/number-stream-2.dat";
    const DL_DATA_ARGS_CFG: &'static str = "../test-collateral/args.cfg";
    const DL_DATA_MODEL_CFG: &'static str = "../test-collateral/mnist_lenet.cfg";
    const DL_DATA_LIST_CFG: &'static str = "../test-collateral/mnist.dataset";
    const DL_DATA_WEIGHTS_CFG: &'static str = "../test-collateral/mnist_lenet.weights";
    const DL_DATA_DATANAMES_CFG: &'static str = "../test-collateral/mnist.names.list";
    const DL_DATA_IMAGE_CFG: &'static str = "../test-collateral/t_00000_c5.png";

    const LOGISTICS_REGRESSION_DATA_PATH: &'static str = "../test-collateral/idash2017/";
    const MACD_DATA_PATH: &'static str = "../test-collateral/macd/";

    static SETUP: Once = Once::new();
    static DEBUG_SETUP: Once = Once::new();
    lazy_static! {
        // This is a semi-hack to test of if the debug is called in the SGX env.
        // In each run this flag should be set false.
        static ref DEBUG_IS_CALLED: AtomicBool = AtomicBool::new(false);
        // A global flag, between the server thread and the client thread in the test_template.
        // If one of the two threads hits an Error, it sets the flag to `false` and
        // thus stops another thread. Without this hack, a failure can cause non-termination.
        static ref CONTINUE_FLAG_HASH: Mutex<HashMap<u32,bool>> = Mutex::new(HashMap::<u32,bool>::new());
        static ref NEXT_TICKET: AtomicU32 = AtomicU32::new(0);
    }

    pub fn setup(proxy_attestation_server_url: String) -> u32 {
        #[allow(unused_assignments)]
        let rst = NEXT_TICKET.fetch_add(1, Ordering::SeqCst);

        SETUP.call_once(|| {
            println!("SETUP.call_once called");
            env_logger::init();
            let _main_loop_handle = std::thread::spawn(|| {
                let mut sys = System::new("Veracruz Proxy Attestation Server");
                println!("spawned thread calling server with url:{:?}", proxy_attestation_server_url);
                #[cfg(feature = "debug")]
                let server =
                    proxy_attestation_server::server::server(
                        proxy_attestation_server_url,
                        CA_CERT,
                        CA_KEY,
                        true)
                        .unwrap();
                #[cfg(not(feature = "debug"))]
                let server =
                    proxy_attestation_server::server::server(
                        proxy_attestation_server_url,
                        CA_CERT,
                        CA_KEY,
                        false)
                        .unwrap();
                sys.block_on(server).unwrap();
            });
        });
        // sleep to wait for the proxy attestation server to start
        std::thread::sleep(std::time::Duration::from_millis(100));
        rst
    }

    pub fn debug_setup() {
        DEBUG_SETUP.call_once(|| {
            std::env::set_var("RUST_LOG", "debug,actix_server=debug,actix_web=debug");
            env_logger::builder()
                // Check if the debug is called.
                .format(|buf, record| {
                    let message = format!("{}", record.args());
                    if record.level() == LevelFilter::Debug
                        && message.contains("Enclave debug message")
                    {
                        DEBUG_IS_CALLED.store(true, Ordering::SeqCst);
                    }
                    writeln!(buf, "[{} {}]: {}", record.target(), record.level(), message)
                })
                .init();
        });
    }


    #[test]
    fn test_deep_learning_server() {
        let result = test_template::<Vec<u8>>(
            DEEP_LEARNING_SERVER_POLICY,
            CLIENT_CERT,
            CLIENT_KEY,
            Some(DEEP_LEARNING_WASM),
            &[("args.cfg", DL_DATA_ARGS_CFG),
                ("mnist_lenet.cfg", DL_DATA_MODEL_CFG),
                ("mnist.dataset", DL_DATA_LIST_CFG),
                ("mnist_lenet.weights", DL_DATA_WEIGHTS_CFG),
                ("mnist.names.list", DL_DATA_DATANAMES_CFG),
                ("t_00000_c5.png", DL_DATA_IMAGE_CFG)],
            &[],
        );
        assert!(result.is_ok(), "error:{:?}", result);
    }






    /// This is the template of test cases for veracruz-server,
    /// ensuring it is a single client policy,
    /// and the client_cert and client_key match the policy
    /// The type T is the return type of the computation
    fn test_template<T: std::fmt::Debug + serde::de::DeserializeOwned>(
        policy_path: &str,
        client_cert_path: &str,
        client_key_path: &str,
        program_path: Option<&str>,
        // Assuming there is a single data provider,
        // yet the client can provision several packages.
        // The list determines the order of which data is sent out, from head to tail.
        // Each element contains the package id (u64) and the path to the data
        data_id_paths: &[(&str, &str)],
        stream_id_paths: &[(&str, &str)],
    ) -> Result<(), VeracruzServerError> {
        info!("### Step 0.  Initialise test configuration.");
        // initialise the pipe
        let (server_tls_tx, client_tls_rx): (
            std::sync::mpsc::Sender<std::vec::Vec<u8>>,
            std::sync::mpsc::Receiver<std::vec::Vec<u8>>,
        ) = std::sync::mpsc::channel();
        let (client_tls_tx, server_tls_rx): (
            std::sync::mpsc::Sender<(u32, std::vec::Vec<u8>)>,
            std::sync::mpsc::Receiver<(u32, std::vec::Vec<u8>)>,
        ) = std::sync::mpsc::channel();

        info!("### Step 1.  Read policy and set up the proxy attestation server.");
        // load the policy, initialise enclave and start tls
        let time_setup = Instant::now();
        let (policy, policy_json, policy_hash) = read_policy(policy_path)?;
        //let debug_flag = policy.debug;
        let ticket = setup(policy.proxy_attestation_server_url().clone());
        info!(
            "             Setup time (μs): {}.",
            time_setup.elapsed().as_micros()
        );
        info!("### Step 2.  Initialise enclave.");
        let time_init = Instant::now();
        let mut veracruz_server = VeracruzServerEnclave::new(&policy_json)?;
        let client_session_id = veracruz_server.new_tls_session().and_then(|id| {
            if id == 0 {
                Err(VeracruzServerError::MissingFieldError("client_session_id"))
            } else {
                Ok(id)
            }
        })?;
        #[cfg(feature = "nitro")]
        let test_target_platform: Platform = Platform::Nitro;
        #[cfg(feature = "sgx")]
        let test_target_platform: Platform = Platform::SGX;
        #[cfg(feature = "tz")]
        let test_target_platform: Platform = Platform::TrustZone;

        info!("             Enclave generated a self-signed certificate:");

        let mut client_session = create_client_test_session(
            client_cert_path,
            client_key_path,
        )?;
        info!(
            "             Initialasation time (μs): {}.",
            time_init.elapsed().as_micros()
        );

        info!("### Step 3.  Spawn Veracruz server thread.");
        let time_server_boot = Instant::now();
        CONTINUE_FLAG_HASH.lock()?.insert(ticket, true);
        let server_loop_handle = thread::spawn(move || {
            server_tls_loop(&mut veracruz_server, server_tls_tx, server_tls_rx, ticket).map_err(
                |e| {
                    CONTINUE_FLAG_HASH.lock().unwrap().insert(ticket, false);
                    e
                },
            )
        });
        info!(
            "             Booting Veracruz server time (μs): {}.",
            time_server_boot.elapsed().as_micros()
        );

        // Need to clone paths to concreate strings,
        // so the ownership can be transferred into a client thread.
        let program_path: Option<String> = program_path.map(|p| p.to_string());
        // Assuming we are using single data provider,
        // yet the client can provision several packages.
        // The list determines the order of which data is sent out, from head to tail.
        // Each element contains the package id (u64) and the path to the data
        let data_id_paths: Vec<_> = data_id_paths
            .iter()
            .map(|(number, path)| (number.to_string(), path.to_string()))
            .collect();
        let stream_id_paths: Vec<_> = stream_id_paths
            .iter()
            .map(|(number, path)| (number.to_string(), path.to_string()))
            .collect();

        // This is a closure, containing instructions from clients.
        // A sperate thread is spawn and direcly call this closure.
        // However if an Error pop up, the thread set the CONTINUE_FLAG to false,
        // hence stopping the server thread.
        let mut client_body = move || {
            info!(
                "### Step 4.  Client provisions program at {:?}.",
                program_path
            );

            //TODO: change to the actually remote filename
            let program_file_name = if let Some(path) = program_path.as_ref() {
                Path::new(path).file_name().unwrap().to_str().unwrap()
            } else {
                "no_program"
            };
            // if there is a program provided
            if let Some(path) = program_path.as_ref() {
                let time_provosion_data = Instant::now();
                check_policy_hash(
                    &policy_hash,
                    client_session_id,
                    &mut client_session,
                    ticket,
                    &client_tls_tx,
                    &client_tls_rx,
                )?;
                check_runtime_manager_hash(&policy,
                                           &client_session,
                                           &test_target_platform)?;

                let response = provision_program(
                    path,
                    client_session_id,
                    &mut client_session,
                    ticket,
                    &client_tls_tx,
                    &client_tls_rx,
                )?;
                info!(
                    "             Client received acknowledgement after sending program: {:?}",
                    transport_protocol::parse_runtime_manager_response(&response)
                );
                info!(
                    "             Provisioning program time (μs): {}.",
                    time_provosion_data.elapsed().as_micros()
                );
            }

            info!("### Step 6.  Data providers provision secret data.");
            for (remote_file_name, data_path) in data_id_paths.iter() {
                info!(
                    "             Data providers provision secret data #{}.",
                    remote_file_name
                );
                let time_data_hash = Instant::now();
                check_policy_hash(
                    &policy_hash,
                    client_session_id,
                    &mut client_session,
                    ticket,
                    &client_tls_tx,
                    &client_tls_rx,
                )?;
                check_runtime_manager_hash(&policy,
                                           &client_session,
                                           &test_target_platform)?;
                info!(
                    "             Data provider hash response time (μs): {}.",
                    time_data_hash.elapsed().as_micros()
                );
                let time_data = Instant::now();
                let response = provision_data(
                    data_path.as_str(),
                    client_session_id,
                    &mut client_session,
                    ticket,
                    &client_tls_tx,
                    &client_tls_rx,
                    remote_file_name,
                )?;
                info!(
                    "             Client received acknowledgement after sending data: {:?},",
                    transport_protocol::parse_runtime_manager_response(&response)
                );
                info!(
                    "             Provisioning data time (μs): {}.",
                    time_data.elapsed().as_micros()
                );
            }
            // If stream_id_paths is NOT empty, we are in streaming mode
            if !stream_id_paths.is_empty() {
                info!("### Step 7.  Stream providers request the program hash.");

                let mut id_vec = Vec::new();
                let mut stream_data_vec = Vec::new();

                for (remote_file_name, data_path) in stream_id_paths.iter() {
                    id_vec.push(remote_file_name);
                    let data = {
                        let mut data_file = std::fs::File::open(data_path)?;
                        let mut data_buffer = std::vec::Vec::new();
                        data_file.read_to_end(&mut data_buffer)?;
                        data_buffer
                    };
                    let decoded_data: Vec<Vec<u8>> = pinecone::from_bytes(&data.as_slice())?;
                    // convert vec of raw stream packages to queue of them
                    stream_data_vec.push(decoded_data);
                }

                check_runtime_manager_hash(&policy,
                                           &client_session,
                                           &test_target_platform)?;

                // Reverse the vec so we can use `pop` for the `first` element of the list.
                // In each round of stream, the loop pops an element from the `stream_data_vec`
                // in the order specified in the package id vec `id_vec`.
                // e.g. if id_vec is [2,1,0], the loop pops stream_data_vec[2] then
                // stream_data_vec[1] and then stream_data_vec[0].
                stream_data_vec.iter_mut().for_each(|e| e.reverse());
                let mut count = 0;
                loop {
                    let next_round_data: Vec<_> = {
                        let next: Vec<_> = stream_data_vec.iter_mut().map(|d| d.pop()).collect();
                        if next.iter().any(|e| e.is_none()) {
                            break;
                        }
                        id_vec
                            .clone()
                            .into_iter()
                            .zip(next.into_iter().flatten())
                            .collect()
                    };
                    info!("------------ Streaming Round # {} ------------", count);
                    count += 1;
                    for (remote_file_name, data) in next_round_data.iter() {
                        let time_stream_hash = Instant::now();
                        check_policy_hash(
                            &policy_hash,
                            client_session_id,
                            &mut client_session,
                            ticket,
                            &client_tls_tx,
                            &client_tls_rx,
                        )?;
                        info!(
                            "             Stream provider hash response time (μs): {}.",
                            time_stream_hash.elapsed().as_micros()
                        );
                        info!(
                            "             Stream provider provision secret data #{}.",
                            remote_file_name
                        );
                        let time_stream = Instant::now();
                        let response = provision_stream(
                            data.as_slice(),
                            client_session_id,
                            &mut client_session,
                            ticket,
                            &client_tls_tx,
                            &client_tls_rx,
                            remote_file_name,
                        )?;
                        info!(
                            "             Stream provider received acknowledgement after sending stream data: {:?},",
                            transport_protocol::parse_runtime_manager_response(&response)
                        );
                        info!(
                            "             Provisioning stream time (μs): {}.",
                            time_stream.elapsed().as_micros()
                        );
                    }
                    info!(
                        "### Step 8.  Result retrievers request program {}.",
                        program_file_name
                    );
                    let time_result_hash = Instant::now();
                    check_policy_hash(
                        &policy_hash,
                        client_session_id,
                        &mut client_session,
                        ticket,
                        &client_tls_tx,
                        &client_tls_rx,
                    )?;
                    check_runtime_manager_hash(&policy,
                                               &client_session,
                                               &test_target_platform)?;
                    info!(
                        "             Result retriever hash response time (μs): {}.",
                        time_result_hash.elapsed().as_micros()
                    );
                    let time_result = Instant::now();
                    info!("             Result retrievers request result.");
                    // NOTE: Fetch result twice on purpose.
                    client_tls_send(
                        &client_tls_tx,
                        &client_tls_rx,
                        client_session_id,
                        &mut client_session,
                        ticket,
                        &transport_protocol::serialize_request_result(program_file_name)?
                            .as_slice(),
                    )
                    .and_then(|response| {
                        // decode the result
                        let response =
                            transport_protocol::parse_runtime_manager_response(&response)?;
                        let response = transport_protocol::parse_result(&response)?;
                        response.ok_or(VeracruzServerError::MissingFieldError(
                            "Result retrievers response",
                        ))
                    })?;
                    let response = client_tls_send(
                        &client_tls_tx,
                        &client_tls_rx,
                        client_session_id,
                        &mut client_session,
                        ticket,
                        &transport_protocol::serialize_request_result(program_file_name)?
                            .as_slice(),
                    )
                    .and_then(|response| {
                        // decode the result
                        let response =
                            transport_protocol::parse_runtime_manager_response(&response)?;
                        let response = transport_protocol::parse_result(&response)?;
                        response.ok_or(VeracruzServerError::MissingFieldError(
                            "Result retrievers response",
                        ))
                    })?;
                    info!(
                        "             Computation result time (μs): {}.",
                        time_result.elapsed().as_micros()
                    );
                    info!("### Step 9.  Client decodes the result.");
                    let result: T = pinecone::from_bytes(&response.as_slice())?;
                    info!("             Client received result: {:?},", result);
                }
                info!("------------ Stream-Result-Next End  ------------");
            } else {
                info!("### Step 7.  NOT in streaming mode.");
                info!(
                    "### Step 8.  Result retrievers request program {}.",
                    program_file_name
                );
                let time_result_hash = Instant::now();
                check_policy_hash(
                    &policy_hash,
                    client_session_id,
                    &mut client_session,
                    ticket,
                    &client_tls_tx,
                    &client_tls_rx,
                )?;

                check_runtime_manager_hash(&policy,
                                           &client_session,
                                           &test_target_platform)?;
                info!(
                    "             Result retriever hash response time (μs): {}.",
                    time_result_hash.elapsed().as_micros()
                );
                let time_result = Instant::now();
                info!("             Result retrievers request result.");
                let response = client_tls_send(
                    &client_tls_tx,
                    &client_tls_rx,
                    client_session_id,
                    &mut client_session,
                    ticket,
                    &transport_protocol::serialize_request_result(program_file_name)?.as_slice(),
                )
                .and_then(|response| {
                    // decode the result
                    let response = transport_protocol::parse_runtime_manager_response(&response)?;
                    let response = transport_protocol::parse_result(&response)?;
                    response.ok_or(VeracruzServerError::MissingFieldError(
                        "Result retrievers response",
                    ))
                })?;
                info!(
                    "             Computation result time (μs): {}.",
                    time_result.elapsed().as_micros()
                );
                info!("### Step 9.  Client decodes the result.");
                let result: T = pinecone::from_bytes(&response.as_slice())?;
                info!("             Client received result: {:?},", result);
            }

            info!("### Step 10. Client shuts down Veracruz.");
            let time_shutdown = Instant::now();
            let response = client_tls_send(
                &client_tls_tx,
                &client_tls_rx,
                client_session_id,
                &mut client_session,
                ticket,
                &transport_protocol::serialize_request_shutdown()?.as_slice(),
            )?;
            info!(
                "             Client received acknowledgment after shutdown request: {:?}",
                transport_protocol::parse_runtime_manager_response(&response)
            );
            info!(
                "             Shutdown time (μs): {}.",
                time_shutdown.elapsed().as_micros()
            );
            Ok::<(), VeracruzServerError>(())
        };

        thread::spawn(move || {
            client_body().map_err(|e| {
                CONTINUE_FLAG_HASH.lock().unwrap().insert(ticket, false);
                e
            })
        })
        .join()
        // double `?` one for join and one for client_body
        .map_err(|e| VeracruzServerError::JoinError(e))??;

        // double `?` one for join and one for client_body
        server_loop_handle
            .join()
            .map_err(|e| VeracruzServerError::JoinError(e))??;
        Ok(())
    }

    /// Auxiliary function: apply functor to all the policy file (json file) in the path
    fn iterate_over_policy(dir_path: &str, f: fn(&str) -> ()) {
        for entry in Path::new(dir_path)
            .read_dir()
            .expect(&format!("invalid dir path:{}", dir_path))
        {
            if let Ok(entry) = entry {
                if let Some(extension_str) = entry
                    .path()
                    .extension()
                    .and_then(|extension_name| extension_name.to_str())
                {
                    // iterate over all the json file
                    if extension_str.eq_ignore_ascii_case("json") {
                        let policy_path = entry.path();
                        if let Some(policy_filename) = policy_path.to_str() {
                            let policy_json = std::fs::read_to_string(policy_filename)
                                .expect(&format!("Cannot open file {}", policy_filename));
                            f(&policy_json);
                        }
                    }
                }
            }
        }
    }

    fn iterate_over_data(dir_path: &str, f: fn(&str) -> ()) {
        let test_collateral_path = Path::new(dir_path);
        for entry in test_collateral_path
            .read_dir()
            .expect(&format!("invalid path:{}", dir_path))
        {
            if let Ok(entry) = entry {
                if let Some(extension_str) = entry
                    .path()
                    .extension()
                    .and_then(|extension_name| extension_name.to_str())
                {
                    // iterate over all the json file
                    if extension_str.eq_ignore_ascii_case("dat") {
                        let data_path = entry.path();
                        if let Some(data_path) = data_path.to_str() {
                            f(data_path);
                        }
                    }
                }
            }
        }
    }

    /// Auxiliary function: read policy file
    fn read_policy(fname: &str) -> Result<(Policy, String, String), VeracruzServerError> {
        let policy_json =
            std::fs::read_to_string(fname).expect(&format!("Cannot open file {}", fname));


        let policy_hash = ring::digest::digest(&ring::digest::SHA256, policy_json.as_bytes());
        let policy_hash_str = hex::encode(&policy_hash.as_ref().to_vec());
        let policy = Policy::from_json(policy_json.as_ref())?;
        Ok((policy, policy_json.to_string(), policy_hash_str))
    }

    /// Auxiliary function: initialise the Veracruz server from policy and open a tls session
    fn init_veracruz_server_and_tls_session(
        policy_json: &str,
    ) -> Result<(VeracruzServerEnclave, u32), VeracruzServerError> {
        let veracruz_server = VeracruzServerEnclave::new(&policy_json)?;

        let one_tenth_sec = std::time::Duration::from_millis(100);
        std::thread::sleep(one_tenth_sec); // wait for the client to start

        veracruz_server.new_tls_session().and_then(|session_id| {
            if session_id != 0 {
                Ok((veracruz_server, session_id))
            } else {
                Err(VeracruzServerError::MissingFieldError("Session id"))
            }
        })
    }

    fn provision_program(
        filename: &str,
        client_session_id: u32,
        client_session: &mut dyn rustls::Session,
        ticket: u32,
        client_tls_tx: &std::sync::mpsc::Sender<(u32, std::vec::Vec<u8>)>,
        client_tls_rx: &std::sync::mpsc::Receiver<std::vec::Vec<u8>>,
    ) -> Result<Vec<u8>, VeracruzServerError> {
        let mut program_file = std::fs::File::open(filename)?;
        let mut program_text = std::vec::Vec::new();

        program_file.read_to_end(&mut program_text)?;

        let serialized_program_text = transport_protocol::serialize_program(
            &program_text,
            Path::new(filename).file_name().unwrap().to_str().unwrap(),
        )?;
        client_tls_send(
            client_tls_tx,
            client_tls_rx,
            client_session_id,
            client_session,
            ticket,
            &serialized_program_text[..],
        )
    }

    fn check_policy_hash(
        expected_policy_hash: &str,
        client_session_id: u32,
        client_session: &mut dyn rustls::Session,
        ticket: u32,
        client_tls_tx: &std::sync::mpsc::Sender<(u32, std::vec::Vec<u8>)>,
        client_tls_rx: &std::sync::mpsc::Receiver<std::vec::Vec<u8>>,
    ) -> Result<(), VeracruzServerError> {
        let serialized_request_policy_hash = transport_protocol::serialize_request_policy_hash()?;
        let response = client_tls_send(
            client_tls_tx,
            client_tls_rx,
            client_session_id,
            client_session,
            ticket,
            &serialized_request_policy_hash[..],
        )?;
        let parsed_response = transport_protocol::parse_runtime_manager_response(&response)?;
        let status = parsed_response.get_status();
        if status != transport_protocol::ResponseStatus::SUCCESS {
            return Err(VeracruzServerError::ResponseError(
                "check_policy_hash parse_runtime_manager_response",
                status,
            ));
        }
        let received_hash = std::str::from_utf8(&parsed_response.get_policy_hash().data)?;
        if received_hash == expected_policy_hash {
            return Ok(());
        } else {
            return Err(VeracruzServerError::MismatchError {
                variable: "request_policy_hash",
                received: received_hash.as_bytes().to_vec(),
                expected: expected_policy_hash.as_bytes().to_vec(),
            });
        }
    }

    fn compare_policy_hash(received: &[u8], policy: &Policy, platform: &Platform) -> bool {
        #[cfg(feature = "debug")]
        {
            // don't check hash because the received hash might be zeros (for nitro, for example)
            return true;
        }
        #[cfg(not(feature = "debug"))]
        {
            let expected = match policy.runtime_manager_hash(platform) {
                Err(_) => return false,
                Ok(data) => data,
            };
            let expected_bytes = match hex::decode(expected) {
                Err(_) => return false,
                Ok(bytes) => bytes,
            };
    
            if &received[..] != expected_bytes.as_slice() {
                return false;
            } else {
                return true;
            }
        }
    }

    fn check_runtime_manager_hash(policy: &Policy,
                                  client_session: &dyn rustls::Session,
                                  test_target_platform: &Platform,
    ) -> Result<(), VeracruzServerError> {
        match client_session.get_peer_certificates() {
            None => {
                return Err(VeracruzServerError::MissingFieldError("NO PEER CERTIFICATES. WTF?"));
            },
            Some(certs) => {
                let ee_cert = webpki::EndEntityCert::from(certs[0].as_ref()).unwrap();
                let ues = ee_cert.unrecognized_extensions();
                // check for OUR extension
                let encoded_extension_id: [u8; 3] = [VERACRUZ_RUNTIME_HASH_EXTENSION_ID[0] * 40 + VERACRUZ_RUNTIME_HASH_EXTENSION_ID[1],
                                                     VERACRUZ_RUNTIME_HASH_EXTENSION_ID[2],
                                                     VERACRUZ_RUNTIME_HASH_EXTENSION_ID[3]];
                match ues.get(&encoded_extension_id[..]) {
                    None => {
                        println!("Our extension is not present. This should be fatal");
                        return Err(VeracruzServerError::MissingFieldError("MY CRAZY CUSTOM EXTENSION AIN'T TERE"));
                    },
                    Some(data) => {
                        let extension_data = data.read_all(VeracruzServerError::MissingFieldError("CAN'T READ MY CRAZY CUSTOM EXTENSION"), |input| {
                            Ok(input.read_bytes_to_end())
                        })?;
                        if !compare_policy_hash(extension_data.as_slice_less_safe(), &policy, test_target_platform) {
                               // The hashes didn't match
                               println!("None of the hashes matched.");
                               return Err(VeracruzServerError::InvalidRuntimeManagerHash);
                        }
                        return Ok(());
                    }
                }
            }
        }
    }

    fn provision_data(
        filename: &str,
        client_session_id: u32,
        client_session: &mut rustls::ClientSession,
        ticket: u32,
        client_tls_tx: &std::sync::mpsc::Sender<(u32, std::vec::Vec<u8>)>,
        client_tls_rx: &std::sync::mpsc::Receiver<std::vec::Vec<u8>>,
        remote_file_name: &str,
    ) -> Result<Vec<u8>, VeracruzServerError> {
        // The client also sends the associated data
        let data = {
            let mut data_file = std::fs::File::open(filename)?;
            let mut data_buffer = std::vec::Vec::new();
            data_file.read_to_end(&mut data_buffer)?;
            data_buffer
        };
        let serialized_data = transport_protocol::serialize_program_data(&data, remote_file_name)?;

        client_tls_send(
            client_tls_tx,
            client_tls_rx,
            client_session_id,
            client_session,
            ticket,
            &serialized_data[..],
        )
    }

    fn provision_stream(
        data: &[u8],
        client_session_id: u32,
        client_session: &mut rustls::ClientSession,
        ticket: u32,
        client_tls_tx: &std::sync::mpsc::Sender<(u32, std::vec::Vec<u8>)>,
        client_tls_rx: &std::sync::mpsc::Receiver<std::vec::Vec<u8>>,
        remote_file_name: &str,
    ) -> Result<Vec<u8>, VeracruzServerError> {
        // The client also sends the associated data
        let serialized_stream = transport_protocol::serialize_stream(data, remote_file_name)?;

        client_tls_send(
            client_tls_tx,
            client_tls_rx,
            client_session_id,
            client_session,
            ticket,
            &serialized_stream[..],
        )
    }

    fn server_tls_loop(
        veracruz_server: &mut dyn veracruz_server::VeracruzServer,
        tx: std::sync::mpsc::Sender<std::vec::Vec<u8>>,
        rx: std::sync::mpsc::Receiver<(u32, std::vec::Vec<u8>)>,
        ticket: u32,
    ) -> Result<(), VeracruzServerError> {
        while *CONTINUE_FLAG_HASH.lock()?.get(&ticket).ok_or(
            VeracruzServerError::MissingFieldError("CONTINUE_FLAG_HASH ticket"),
        )? {
            let received = rx.try_recv();
            let (session_id, received_buffer) = received.unwrap_or_else(|_| (0, Vec::new()));

            if received_buffer.len() > 0 {
                let (active_flag, output_data_option) =
                    veracruz_server.tls_data(session_id, received_buffer)?;
                let output_data = output_data_option.unwrap_or_else(|| Vec::new());
                for output in output_data.iter() {
                    if output.len() > 0 {
                        tx.send(output.clone())?;
                    }
                }
                if !active_flag {
                    return Ok(());
                }
            }
        }
        Err(VeracruzServerError::DirectStrError(
            "No message arrives server",
        ))
    }

    fn client_tls_send(
        tx: &std::sync::mpsc::Sender<(u32, std::vec::Vec<u8>)>,
        rx: &std::sync::mpsc::Receiver<std::vec::Vec<u8>>,
        session_id: u32,
        session: &mut dyn rustls::Session,
        ticket: u32,
        send_data: &[u8],
    ) -> Result<Vec<u8>, VeracruzServerError> {
        session.write_all(&send_data)?;

        let mut output: std::vec::Vec<u8> = std::vec::Vec::new();

        session.write_tls(&mut output)?;

        tx.send((session_id, output))?;

        while *CONTINUE_FLAG_HASH.lock()?.get(&ticket).ok_or(
            VeracruzServerError::MissingFieldError("CONTINUE_FLAG_HASH ticket"),
        )? {
            let received = rx.try_recv();

            if received.is_ok() && (!session.is_handshaking() || session.wants_read()) {
                let received = received?;

                let mut slice = &received[..];
                session.read_tls(&mut slice)?;
                session.process_new_packets()?;

                let mut received_buffer: std::vec::Vec<u8> = std::vec::Vec::new();

                let num_bytes = session.read_to_end(&mut received_buffer)?;
                if num_bytes > 0 {
                    return Ok(received_buffer);
                }
            } else if session.wants_write() {
                let mut output: std::vec::Vec<u8> = std::vec::Vec::new();
                session.write_tls(&mut output)?;
                let _res = tx.send((session_id, output))?;
            }
        }
        Err(VeracruzServerError::DirectStrError(
            "Terminate due to server crash",
        ))
    }

    fn create_client_test_session(
        client_cert_filename: &str,
        client_key_filename: &str,
    ) -> Result<rustls::ClientSession, VeracruzServerError> {
        let client_cert = read_cert_file(client_cert_filename)?;

        let client_priv_key = read_priv_key_file(client_key_filename)?;

        let proxy_service_cert = {
            let data = std::fs::read(CA_CERT).unwrap();
            let certs = rustls::internal::pemfile::certs(&mut data.as_slice()).unwrap();
            certs[0].clone()
        };
        let mut client_config = rustls::ClientConfig::new();
        let mut client_cert_vec = std::vec::Vec::new();
        client_cert_vec.push(client_cert);
        client_config.set_single_client_cert(client_cert_vec, client_priv_key);
        client_config
            .root_store
            .add(&proxy_service_cert).unwrap();

        let dns_name = webpki::DNSNameRef::try_from_ascii_str("ComputeEnclave.dev")?;
        Ok(rustls::ClientSession::new(
            &std::sync::Arc::new(client_config),
            dns_name,
        ))
    }

    fn read_cert_file(filename: &str) -> Result<rustls::Certificate, VeracruzServerError> {
        let mut cert_file = std::fs::File::open(filename)?;
        let mut cert_buffer = std::vec::Vec::new();
        cert_file.read_to_end(&mut cert_buffer)?;
        let mut cursor = std::io::Cursor::new(cert_buffer);
        let certs = rustls::internal::pemfile::certs(&mut cursor)
            .map_err(|_| VeracruzServerError::TLSUnspecifiedError)?;
        if certs.len() == 0 {
            Err(VeracruzServerError::InvalidLengthError("certs.len()", 1))
        } else {
            Ok(certs[0].clone())
        }
    }

    fn read_priv_key_file(filename: &str) -> Result<rustls::PrivateKey, VeracruzServerError> {
        let mut key_file = std::fs::File::open(filename)?;
        let mut key_buffer = std::vec::Vec::new();
        key_file.read_to_end(&mut key_buffer)?;
        let mut cursor = std::io::Cursor::new(key_buffer);
        let rsa_keys = rustls::internal::pemfile::rsa_private_keys(&mut cursor)
            .map_err(|_| VeracruzServerError::TLSUnspecifiedError)?;
        Ok(rsa_keys[0].clone())
    }
}
