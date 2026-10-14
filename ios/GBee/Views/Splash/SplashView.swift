import SwiftUI

struct SplashView: View {
    @State private var scale: CGFloat = 0.5
    @State private var opacity: Double = 0
    @State private var isActive = false

    var body: some View {
        ZStack {
            Color.nightVoid
                .ignoresSafeArea()

            VStack(spacing: GBeeDesign.spacingLG) {
                Spacer()

                // Bee character placeholder
                ZStack {
                    RoundedRectangle(cornerRadius: GBeeDesign.radiusXL)
                        .fill(Color.goldenSaplight)
                        .frame(width: 120, height: 120)

                    Text("🐝")
                        .font(.system(size: 64))
                }
                .scaleEffect(scale)
                .opacity(opacity)

                VStack(spacing: GBeeDesign.spacingSM) {
                    Text("GBee Emulator")
                        .font(.system(size: GBeeDesign.fontDisplay, weight: .bold, design: .rounded))
                        .foregroundColor(.goldenSaplight)

                    Text("GBA Emulator")
                        .font(.system(size: GBeeDesign.fontBody, weight: .light))
                        .foregroundColor(.pineGlowMist)
                }
                .opacity(opacity)

                Spacer()

                Text("Bzzt! Let's play!")
                    .font(.system(size: GBeeDesign.fontCaption))
                    .foregroundColor(.amberResin)
                    .opacity(opacity)
                    .padding(.bottom, GBeeDesign.spacingXXL)
            }
        }
        .onAppear {
            withAnimation(.easeOut(duration: 0.5)) {
                scale = 1.0
                opacity = 1.0
            }

            DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
                isActive = true
            }
        }
        .fullScreenCover(isPresented: $isActive) {
            ROMBrowserView()
        }
    }
}
