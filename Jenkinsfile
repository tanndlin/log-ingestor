def setStatus(String state, String description) {
    // Shell expands $GITHUB_TOKEN so the secret is not interpolated by Groovy
    sh """
    curl -L \
    -X POST \
    -H "Accept: application/vnd.github+json" \
    -H "Authorization: Bearer \$GITHUB_TOKEN" \
    -H "X-GitHub-Api-Version: 2022-11-28" \
    https://api.github.com/repos/tanndlin/log-ingestor/statuses/\$GIT_COMMIT \
    -d '{"state":"${state}","description":"${description}","context":"Jenkins"}'
    """
}

pipeline {
    agent any

    // Overlapping deploys race on `docker compose up`, and an older build finishing last
    // can roll the services back behind migrations a newer one already applied.
    options {
        disableConcurrentBuilds()
    }

    environment {
        GITHUB_TOKEN = credentials('GITHUB_TOKEN')
        DOCKER_VOLS = '-v jenkins_jenkins_home:/var/jenkins_home -v cargo-registry-cache:/usr/local/cargo/registry -v pnpm-store-cache:/root/.local/share/pnpm/store'
        NODE_IMAGE = 'log-ingestor-node-ci:26'
        RUST_IMAGE = 'log-ingestor-rust-ci:1.99'
        // No DB reachable in CI, so sqlx::query! checks against the committed .sqlx/ cache.
        SQLX_OFFLINE = 'true'
    }

    stages {
        stage('Checkout') {
            steps {
                checkout scm
                setStatus('pending', 'Build in progress')
            }
        }

        // Viewer and Rust workspace don't depend on each other, so run them side by side.
        // Within each, cheap checks run before the slow builds.
        stage('CI') {
            parallel {
                stage('Viewer') {
                    stages {
                        stage('Prepare Node Image') {
                            steps {
                                sh 'docker build -q -t $NODE_IMAGE - < log-viewer/ci.Dockerfile'
                            }
                        }

                        stage('Install Viewer') {
                            steps {
                                sh '''
                                docker run --rm $DOCKER_VOLS -w $WORKSPACE/log-viewer $NODE_IMAGE \
                                    sh -c "pnpm install --frozen-lockfile"
                                '''
                            }
                        }

                        stage('Build Viewer') {
                            steps {
                                sh '''
                                docker run --rm $DOCKER_VOLS -w $WORKSPACE/log-viewer $NODE_IMAGE \
                                    sh -c "pnpm build"
                                '''
                            }
                        }
                    }
                }

                stage('Backend') {
                    stages {
                        stage('Prepare Rust Image') {
                            steps {
                                sh 'docker build -q -t $RUST_IMAGE - < ci.Dockerfile'
                            }
                        }

                        stage('Format Backend') {
                            steps {
                                catchError(buildResult: 'FAILURE', stageResult: 'FAILURE') {
                                    sh '''
                                    docker run --rm $DOCKER_VOLS -e SQLX_OFFLINE -w $WORKSPACE $RUST_IMAGE \
                                        sh -c "cargo fmt --all -- --check"
                                    '''
                                }
                            }
                        }

                        stage('Lint Backend') {
                            steps {
                                catchError(buildResult: 'FAILURE', stageResult: 'FAILURE') {
                                    sh '''
                                    docker run --rm $DOCKER_VOLS -e SQLX_OFFLINE -w $WORKSPACE $RUST_IMAGE \
                                        sh -c "cargo clippy --workspace --all-targets -- -D warnings"
                                    '''
                                }
                            }
                        }

                        stage('Build Backend') {
                            steps {
                                sh '''
                                docker run --rm $DOCKER_VOLS -e SQLX_OFFLINE -w $WORKSPACE $RUST_IMAGE \
                                    sh -c "cargo build --release --workspace"
                                '''
                            }
                        }

                        stage('Test Backend') {
                            steps {
                                sh '''
                                docker run --rm $DOCKER_VOLS -e SQLX_OFFLINE -w $WORKSPACE $RUST_IMAGE \
                                    sh -c "cargo test --release --workspace"
                                '''
                            }
                        }
                    }
                }
            }
        }

        stage('Deploy') {
            when {
                allOf {
                    expression { env.GIT_BRANCH == 'master' || env.GIT_BRANCH == 'origin/master' }
                    expression { currentBuild.currentResult == 'SUCCESS' }
                }
            }
            environment {
                POSTGRES_PORT = '5435'
                QUERIER_PORT = '1336'
                TCP_PORT = '1337'
                HTTP_PORT = '1338'
                VIEWER_PORT = '1335'
            }
            steps {
                sh 'docker compose -f docker-compose.yml -p log-ingestor up -d --build --remove-orphans'
            }
        }
    }

    post {
        success {
            setStatus('success', 'Build succeeded')
        }
        failure {
            setStatus('failure', 'Build failed')
        }
    }
}
