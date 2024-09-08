#[cfg(test)]
mod tests {
	use crate::{
		connection::Error,
		handshake::{server::Response, Client, Server, ServerResponse},
		Receiver,
	};
	use std::{
		net::SocketAddr,
		sync::{
			atomic::{AtomicUsize, Ordering},
			Arc,
		},
	};
	use tokio::net::TcpStream;
	use tokio_stream::StreamExt;
	use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

	struct TestData<'a> {
		messages: Vec<&'a str>,
        total_len: usize,
		compression: bool,
		do_reads: bool,
		do_spawn: bool,
        slow: bool,
        binary: bool, // Dont write messages, write messages of parseInt(message) bytes
	}

	async fn client(addr: SocketAddr, counter: &Arc<AtomicUsize>, total_len: &Arc<AtomicUsize>, _compression: bool) {
		// Read from websocket until closed...
		let socket = tokio::net::TcpStream::connect(addr).await.unwrap();

		// Then we configure the client handshake.
		let mut client = Client::new(socket.compat(), "localhost", "/");

		#[cfg(feature = "deflate")]
		if _compression {
			let deflate = crate::extension::deflate::Deflate::new(crate::Mode::Client);
			client.add_extension(Box::new(deflate));
		}

		// And finally we perform the handshake and handle the result.
		let (mut sender, mut receiver) = match client.handshake().await.unwrap() {
			ServerResponse::Accepted { .. } => client.into_builder().finish(),
			ServerResponse::Redirect { status_code: _, location: _ } => unimplemented!("follow location URL"),
			ServerResponse::Rejected { status_code: _ } => unimplemented!("handle failure"),
		};

		// ... and receive data.
		let mut data = Vec::new();
		let mut count: usize = 0;
        let mut len: usize = 0;
		loop {
			data.clear();
			match receiver.receive_data(&mut data).await {
				Ok(data_type) => {
					if data_type.is_text() {
						log::info!("Client received text: {}", std::str::from_utf8(&data).unwrap());
					} else {
						log::info!("Client received binary len {}", data.len());
					}
					count += 1;
                    len += data.len();
				}
				Err(e) => {
					match e {
						Error::Closed => {}
						_ => {
							log::error!("Client error: {e}");
						}
					}
					break;
				}
			}
		}
		let _ = sender.close().await; // might already be closed
		counter.store(count, Ordering::Relaxed);
        total_len.store(len, Ordering::Relaxed);
	}

	async fn loop_read(
		mut receiver: Receiver<Compat<TcpStream>>,

		mut rx: tokio::sync::broadcast::Receiver<()>,
	) -> Result<(), Error> {
		let mut buf = Vec::new();

		loop {
			tokio::select! { biased;
				r = receiver.receive_data(&mut buf) => {
					// This shouldn't happen...
					match r {
						Ok(r) => { log::error!("Received {:02X?}", r);}
						Err(e) => { log::error!("Received error {e}");}
					}
				}
				_r = rx.recv() => {
					return Ok(());
				}
			}
		}
	}

	async fn stream_test<'a>(data: TestData<'a>) {
		// First, we listen for incoming connections.
		let listener = tokio::net::TcpListener::bind("localhost:0").await.unwrap();
		let address = listener.local_addr().unwrap();
		let counter = Arc::new(AtomicUsize::new(0));
		let counter_clone = counter.clone();
		let total_len = Arc::new(AtomicUsize::new(0));
		let total_len_clone = total_len.clone();
		let message_count = data.messages.len();

		let h = tokio::spawn(async move {
			client(address, &counter_clone, &total_len_clone, data.compression).await;
		});

		let mut incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

		if let Some(socket) = incoming.next().await {
			// For each incoming connection we perform a handshake.
			let socket = socket.unwrap();
			let mut server = Server::new(socket.compat());

			#[cfg(feature = "deflate")]
			if data.compression {
				let deflate = crate::extension::deflate::Deflate::new(crate::Mode::Server);
				log::info!("deflate = {:?}", deflate);
				server.add_extension(Box::new(deflate));
			}
			let websocket_key = {
				let req = server.receive_request().await.unwrap();
				req.key()
			};

			// Here we accept the client unconditionally.
			let accept = Response::Accept { key: websocket_key, protocol: None };
			server.send_response(&accept).await.unwrap();
			log::info!("Server = {:?}", server);

			// And we can finally transition to a websocket connection.
			let (mut sender, mut receiver) = server.into_builder().finish();

			let mut buf = Vec::new();

			if data.binary {
				for line in data.messages {
					buf.clear();
                    if data.slow {
                        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    }

                    let len = line.parse::<usize>().unwrap();
                    let mut data: Vec<u8> = Vec::with_capacity(len);
                    for i in 0..len {
                        data.push(i as u8);
                    }
                    assert!(data.len() == len);

					tokio::select! { biased;
						r = receiver.receive_data(&mut buf) => {
							// This shouldn't happen...
							match r {
								Ok(r) => { log::error!("Received {:02X?}", r);}
								Err(e) => { log::error!("Received error {e}");}
							}
						}
						r = sender.send_binary_mut(data) => {
							r.unwrap();
							sender.flush().await.unwrap();
						}
					}
				}
			} else if data.do_spawn {
				let (tx, rx) = tokio::sync::broadcast::channel::<()>(1);
				let j = tokio::spawn(async {
					loop_read(receiver, rx).await.unwrap();
				});

				for line in data.messages {
					tokio::select! { biased;
						r = sender.send_text(line) => {
							r.unwrap();
							sender.flush().await.unwrap();
						}
					}
				}
				tx.send(()).unwrap();
				j.await.unwrap();
			} else if data.do_reads {
				for line in data.messages {
					buf.clear();
                    if data.slow {
                        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    }

					tokio::select! { biased;
						r = receiver.receive_data(&mut buf) => {
							// This shouldn't happen...
							match r {
								Ok(r) => { log::error!("Received {:02X?}", r);}
								Err(e) => { log::error!("Received error {e}");}
							}
						}
						r = sender.send_text(line) => {
							r.unwrap();
							sender.flush().await.unwrap();
						}
					}
				}
			} else {
				for line in data.messages {
					tokio::select! { biased;
						r = sender.send_text(line) => {
							r.unwrap();
							sender.flush().await.unwrap();
						}
					}
				}
			}

			sender.close().await.unwrap();
			h.await.unwrap();
            log::info!("counter = {} / {}", counter.load(Ordering::Relaxed), message_count);
			assert!(counter.load(Ordering::Relaxed) == message_count);
            log::info!("total_len = {} / {}", total_len.load(Ordering::Relaxed), data.total_len);
            assert!(total_len.load(Ordering::Relaxed) == data.total_len);
		}
	}

	const MESSAGES1: [&str; 18] = [
		"Call me Ishmael.",
		"Some years ago- never mind how long precisely- having little or no money in my purse,",
		"and nothing particular to interest me on shore,",
		"I thought I would sail about a little and see the watery part of the world.",
		"It is a way I have of driving off the spleen and regulating the circulation.",
		"Whenever I find myself growing grim about the mouth;",
		"whenever it is a damp, drizzly November in my soul;",
		"whenever I find myself involuntarily pausing before coffin warehouses,",
		"and bringing up the rear of every funeral I meet;",
		"and especially whenever my hypos get such an upper hand of me,",
		"that it requires a strong moral principle to prevent me from deliberately stepping into the street,",
		"and methodically knocking people's hats off- then,",
		"I account it high time to get to sea as soon as I can.",
		"This is my substitute for pistol and ball.",
		" With a philosophical flourish Cato throws himself upon his sword; I quietly take to the ship.",
		"There is nothing surprising in this.",
		"If they but knew it, almost all men in their degree, some time or other,",
		"cherish very nearly the same feelings towards the ocean with me.",
	];

	#[tokio::test]
	async fn stream_single_thead_no_compression_no_read_moby() {
		let data: TestData =
			TestData { messages: MESSAGES1.to_vec(), total_len: 1094, compression: false, do_reads: false, do_spawn: false, slow: false, binary: false };

		stream_test(data).await
	}

	#[tokio::test]
	async fn stream_single_thread_no_compression_moby() {
		let data: TestData =
			TestData { messages: MESSAGES1.to_vec(), total_len: 1094, compression: false, do_reads: true, do_spawn: false, slow: false, binary: false };

		stream_test(data).await
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn stream_no_compression_moby() {
		let data: TestData =
			TestData { messages: MESSAGES1.to_vec(), total_len: 1094, compression: false, do_reads: true, do_spawn: false, slow: false, binary: false };

		stream_test(data).await
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn stream_no_compression_slow_moby() {
		let data: TestData =
			TestData { messages: MESSAGES1.to_vec(), total_len: 1094, compression: false, do_reads: true, do_spawn: false, slow: true, binary: false };

		stream_test(data).await
	}

	
	#[tokio::test(flavor = "multi_thread")]
	async fn stream_no_compression_spawn_moby() {
		let data: TestData =
			TestData { messages: MESSAGES1.to_vec(), total_len: 1094, compression: false, do_reads: true, do_spawn: true, slow: false, binary: false  };

		stream_test(data).await
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn stream_no_compression_no_read_moby() {
		let data: TestData =
			TestData { messages: MESSAGES1.to_vec(), total_len: 1094, compression: false, do_reads: false, do_spawn: false, slow: false, binary: false  };

		stream_test(data).await
	}

	#[cfg(feature = "deflate")]
	#[tokio::test(flavor = "multi_thread")]
	async fn stream_with_compression_moby() {
		let data: TestData =
			TestData { messages: MESSAGES1.to_vec(), total_len: 1094, compression: true, do_reads: true, do_spawn: false, slow: false , binary: false };

		stream_test(data).await
	}

	const MESSAGES2: [&str; 18] = [
		"{'id':'FirmwareVersion','name':'Firmware version','stringValue':'Oct 26 2012 17:02:39 57'}",
		"{'id':'BearingAlignment','name':'Bearing alignment','value':0}",
		"{'id':'SideLobeSuppression','name':'Side lobe suppression','value':29,'auto':true}",
		"{'id':'SerialNumber','name':'Serial Number','stringValue':'1403302452'}",
		"{'id':'TargetBoost','name':'Target boost','value':0,'description':'Off'}",
		"{'id':'Rain','name':'Rain clutter','value':0}",
		"{'id':'AntennaHeight','name':'Antenna height','value':500}",
		"{'id':'SeaState','name':'Sea state','value':2,'description':'Rough'}",
		"{'id':'InterferenceRejection','name':'Interference rejection','value':2,'description':'Medium'}",
		"{'id':'Range','name':'Range','value':463}",
		"{'id':'OperatingHours','name':'Operating hours','value':1233}",
		"{'id':'Status','name':'Status','value':1,'description':'Standby'}",
		"{'id':'Gain','name':'Gain','value':62}",
		"{'id':'ScanSpeed','name':'Fast scan','value':1,'description':'Fast'}",
		"{'id':'NoiseRejection','name':'Noise rejection','value':1,'description':'Low'}",
		"{'id':'TargetExpansion','name':'Target expansion','value':0,'description':'Off'}",
		"{'id':'TargetSeparation','name':'Target separation','value':0,'description':'Off'}",
		"{'id':'Sea','name':'Sea clutter','value':0,'auto':false}",
	];

	#[tokio::test(flavor = "multi_thread")]
	async fn stream_no_compression_json() {
		let data: TestData =
			TestData { messages: MESSAGES2.to_vec(), total_len: 1212, compression: false, do_reads: true, do_spawn: false, slow: false, binary: false  };

		stream_test(data).await
	}

	#[cfg(feature = "deflate")]
	#[tokio::test(flavor = "multi_thread")]
	async fn stream_with_compression_json() {
		let data: TestData =
			TestData { messages: MESSAGES2.to_vec(), total_len: 1212, compression: true, do_reads: true, do_spawn: false, slow: false, binary: false };

		stream_test(data).await
	}

	#[cfg(feature = "deflate")]
	#[tokio::test(flavor = "multi_thread")]
	async fn stream_with_compression_slow_json() {
		let data: TestData =
			TestData { messages: MESSAGES2.to_vec(), total_len: 1212, compression: true, do_reads: true, do_spawn: false, slow: true, binary: false };

		stream_test(data).await
	}

	const MESSAGES3: [&str; 5] = [
		"256",
		"1024",
		"16384",
		"65536",
		"1048576",
	];

	#[tokio::test(flavor = "multi_thread")]
	async fn stream_no_compression_binary() {
		let data: TestData =
			TestData { messages: MESSAGES3.to_vec(), total_len: 256 + 1024 + 16384 + 65536 + 1_048_576, compression: false, do_reads: true, do_spawn: false, slow: false, binary: true  };

		stream_test(data).await
	}
}
