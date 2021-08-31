mkdir -p ../sdk/rust-examples/dl-server/target/wasm32-wasi/release/ && sudo cp ../../veracruz-examples/deep-learning-server/dl-server.wasm ../sdk/rust-examples/dl-server/target/wasm32-wasi/release/dl-server.wasm
sudo cp ../../veracruz-examples/deep-learning-server/args.cfg ../sdk/datasets/args.cfg
sudo cp ../../veracruz-examples/deep-learning-server/cfg/mnist_lenet.cfg ../sdk/datasets/mnist_lenet.cfg
sudo cp ../../veracruz-examples/deep-learning-server/cfg/mnist.dataset ../sdk/datasets/mnist.dataset
sudo cp ../../veracruz-examples/deep-learning-server/model/mnist_lenet_1.weights ../sdk/datasets/mnist_lenet.weights
sudo cp ../../veracruz-examples/deep-learning-server/data/mnist/mnist.names.list ../sdk/datasets/mnist.names.list
sudo cp ../../veracruz-examples/deep-learning-server/data/mnist/images_client1/t_00000_c5.png ../sdk/datasets/t_00000_c5.png
