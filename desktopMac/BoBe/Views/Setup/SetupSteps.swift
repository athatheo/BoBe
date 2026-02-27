import AppKit
import SwiftUI

// MARK: - Setup Step Views

extension SetupWizard {

    // MARK: - Choose Mode

    var chooseModeView: some View {
        VStack(spacing: 16) {
            Text("Choose your AI model").font(.headline).foregroundStyle(theme.colors.text)

            ForEach(ModelSize.allCases, id: \.self) { size in
                modelCard(size)
            }

            Button(busy ? "Checking connection..." : "Continue") { startLocalSetup() }
                .buttonStyle(.borderedProminent).tint(theme.colors.primary)
                .controlSize(.large).disabled(busy)

            DisclosureGroup("Or use a cloud LLM") { cloudOptions }
                .font(.subheadline).foregroundStyle(theme.colors.textMuted).padding(.top, 8)
        }
    }

    func modelCard(_ size: ModelSize) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text(size.displayName).font(.system(size: 14, weight: .semibold)).foregroundStyle(theme.colors.text)
                Text(size.sizeDescription).font(.system(size: 11)).foregroundStyle(theme.colors.textMuted)
                Text(size.description).font(.system(size: 11)).foregroundStyle(theme.colors.textMuted)
            }
            Spacer()
            Image(systemName: selectedModel == size ? "checkmark.circle.fill" : "circle")
                .foregroundStyle(selectedModel == size ? theme.colors.primary : theme.colors.border)
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(theme.colors.surface)
                .stroke(selectedModel == size ? theme.colors.primary : theme.colors.border, lineWidth: 1)
        )
        .onTapGesture { selectedModel = size }
    }

    var cloudOptions: some View {
        VStack(alignment: .leading, spacing: 10) {
            Picker("Provider", selection: $cloudProvider) {
                ForEach(CloudProvider.allCases, id: \.self) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.menu)
            .onChange(of: cloudProvider) { _, p in cloudModel = p.defaultModel }

            SecureField("API Key", text: $apiKey).textFieldStyle(.roundedBorder)

            if cloudProvider == .azure {
                TextField("Endpoint URL", text: $cloudEndpoint).textFieldStyle(.roundedBorder)
            }
            TextField("Model", text: $cloudModel).textFieldStyle(.roundedBorder)

            Button(busy ? "Configuring..." : "Continue with cloud LLM") { startCloudSetup() }
                .buttonStyle(.bordered).disabled(apiKey.isEmpty || busy)
        }
        .padding(.top, 4)
        .onAppear { cloudModel = cloudProvider.defaultModel }
    }

    // MARK: - Downloading

    var downloadingView: some View {
        VStack(spacing: 16) {
            HStack(spacing: 20) {
                stepDot(label: "Engine", done: step != .downloadingEngine)
                stepDot(label: "Model", done: step != .downloadingEngine && step != .downloadingModel)
                stepDot(label: "Features", done: false)
            }

            ProgressView(value: progressPercent, total: 100).progressViewStyle(.linear).frame(width: 300)

            Text(progressMessage).font(.caption).foregroundStyle(theme.colors.textMuted)
                .lineLimit(2).multilineTextAlignment(.center)

            Text("\(Int(progressPercent))%")
                .font(.system(size: 24, weight: .bold, design: .monospaced))
                .foregroundStyle(theme.colors.primary)
        }
    }

    func stepDot(label: String, done: Bool) -> some View {
        VStack(spacing: 4) {
            Circle().fill(done ? theme.colors.primary : theme.colors.border).frame(width: 12, height: 12)
                .overlay { if done { Image(systemName: "checkmark").font(.system(size: 7, weight: .bold)).foregroundStyle(.white) } }
            Text(label).font(.system(size: 9)).foregroundStyle(theme.colors.textMuted)
        }
    }

    // MARK: - Capture Setup

    var captureSetupView: some View {
        VStack(spacing: 14) {
            Text("Screen Capture").font(.headline)
            Text("BoBe watches your screen to understand what you're working on.")
                .font(.subheadline).foregroundStyle(theme.colors.textMuted).multilineTextAlignment(.center)

            featureCard(
                title: "Screen Recording Permission",
                description: "Required for screen analysis. Grant in System Settings.",
                granted: screenPermission == "granted",
                badge: screenPermission == "granted" ? "Granted" : "Not Set"
            ) {
                if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture") {
                    NSWorkspace.shared.open(url)
                }
            }

            if let visionModel = selectedModel.separateVisionModel {
                featureCard(
                    title: "Vision Model (\(visionModel))",
                    description: "Analyzes screen content. ~6 GB download.",
                    granted: visionDownloaded,
                    badge: visionDownloaded ? "Downloaded" : (busy ? "Downloading..." : "Not Downloaded")
                ) {
                    downloadVisionModel()
                }
                if busy {
                    ProgressView(value: progressPercent, total: 100).progressViewStyle(.linear)
                    Text(progressMessage).font(.caption).foregroundStyle(theme.colors.textMuted)
                }
            } else {
                featureCard(
                    title: "Vision Model",
                    description: "Already included — your LLM handles vision too.",
                    granted: true, badge: "Included"
                )
            }

            Text("Screen capture can be enabled later in Settings.")
                .font(.caption).foregroundStyle(theme.colors.textMuted)

            HStack(spacing: 12) {
                Button("Skip") { skipCapture() }
                    .buttonStyle(.bordered).foregroundStyle(theme.colors.textMuted)
                Button("Continue") { step = .complete }
                    .buttonStyle(.borderedProminent).tint(theme.colors.primary)
                    .disabled(
                        screenPermission != "granted"
                            || (selectedModel.separateVisionModel != nil && !visionDownloaded && !busy)
                    )
            }
        }
    }

    // MARK: - Complete

    var completeView: some View {
        VStack(spacing: 16) {
            Image(systemName: "checkmark.circle.fill").font(.system(size: 48)).foregroundStyle(.green)
            Text("All set! BoBe is ready.").font(.headline)

            VStack(alignment: .leading, spacing: 8) {
                summaryRow(icon: "checkmark.circle.fill", color: .green,
                           text: "AI Model: \(selectedModel.displayName)")
                summaryRow(
                    icon: captureSkipped ? "exclamationmark.triangle.fill" : "checkmark.circle.fill",
                    color: captureSkipped ? .orange : .green,
                    text: captureSkipped ? "Screen Capture: Disabled (skipped)" : "Screen Capture: Enabled"
                )
            }
            .padding(16)
            .background(RoundedRectangle(cornerRadius: 10).fill(theme.colors.surface))

            if useCloud {
                Text("Your data stays local. Messages are sent to the cloud API.")
                    .font(.caption).foregroundStyle(theme.colors.textMuted)
            } else {
                Text("Everything runs locally on your Mac.")
                    .font(.caption).foregroundStyle(theme.colors.textMuted)
            }

            Button("Get Started") { completeSetup() }
                .buttonStyle(.borderedProminent).tint(theme.colors.primary).controlSize(.large)
        }
    }

    func summaryRow(icon: String, color: Color, text: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: icon).foregroundStyle(color).font(.system(size: 14))
            Text(text).font(.system(size: 13)).foregroundStyle(theme.colors.text)
        }
    }

    // MARK: - Error

    var errorView: some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle.fill").font(.system(size: 40)).foregroundStyle(.red)
            Text("Setup Failed").font(.headline)
            Text(errorMessage).foregroundStyle(theme.colors.textMuted).multilineTextAlignment(.center)
            Button("Retry") { step = .chooseMode; progressPercent = 0; progressMessage = "" }
                .buttonStyle(.borderedProminent).tint(theme.colors.primary)
        }
    }

    // MARK: - Shared Components

    func featureCard(
        title: String, description: String, granted: Bool, badge: String,
        action: (() -> Void)? = nil
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(title).font(.system(size: 13, weight: .semibold))
                Spacer()
                Text(badge)
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(granted ? theme.colors.secondary : theme.colors.textMuted)
                    .padding(.horizontal, 8).padding(.vertical, 2)
                    .background(RoundedRectangle(cornerRadius: 8)
                        .fill(granted ? theme.colors.secondary.opacity(0.15) : theme.colors.surface))
            }
            Text(description).font(.system(size: 11)).foregroundStyle(theme.colors.textMuted)
            if let action, !granted {
                Button("Set Up") { action() }
                    .font(.system(size: 11, weight: .medium)).foregroundStyle(theme.colors.primary).buttonStyle(.plain)
            }
        }
        .padding(12)
        .background(RoundedRectangle(cornerRadius: 10).fill(theme.colors.surface))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(theme.colors.border, lineWidth: 1))
    }
}
